// The `debit_factor` column on [acc.*] tables: a take of x from an account with
// factor f charges x * f to it.
//
// The contract:
// 1. CHARGE: the balance, `debits`, `use` and roll_cap's banked debits all move
//    by x * f, so allocation = balance + use is unchanged by a take.
// 2. LIMIT: a take may draw balance / f of water; an order may draw
//    (balance - committed) / f, and is committed as order * f.
// 3. WATER STAYS WATER: the node's order, order_due and diversion record water,
//    never the amount charged.
// 4. Each account in a combination is charged at its own factor; order_accounts
//    are charged at theirs.
// 5. LOAD: the factor must be a finite, normal number greater than 0. 1 / f is precomputed at load and
//    survives a rerun.
use crate::io::ini_model_io::IniModelIO;

fn load(ini: &str) -> crate::model::Model {
    let mut model = IniModelIO::read_model_string(ini).expect("model should load");
    model.configure().expect("model should configure");
    model
}

fn run(ini: &str) -> crate::model::Model {
    let mut model = load(ini);
    model.run().expect("simulation should run");
    model
}

fn series(model: &mut crate::model::Model, name: &str) -> Vec<f64> {
    let idx = model.data_cache.get_series_idx(name, false)
        .unwrap_or_else(|| panic!("no series '{}'", name));
    model.data_cache.series[idx].values.clone()
}

fn balance(model: &crate::model::Model, name: &str) -> f64 {
    let idx = model.account_manager.get_account_idx(name).unwrap();
    model.account_manager.get_account_balance(idx)
}

/// inflow (500) -> unregulated user -> blackhole, three days
fn unregulated_model(acc_rows: &str, demand: f64, accounts_line: &str, extra: &str) -> String {
    format!(r#"
[kalix]
start = 2020-01-01
end = 2020-01-03

[acc.g]
accounts = name, size, initial, debit_factor,
{acc_rows}
{extra}
[node.src]
type = inflow
loc = 0, 0
inflow = 500
ds_1 = user

[node.user]
type = unregulated_user
loc = 0, 10
demand = {demand}
{accounts_line}
ds_1 = sink

[node.sink]
type = blackhole
loc = 0, 20

[outputs]
node.user.diversion
acc.a.closing_balance
acc.a.debits
acc.a.use
acc.a.allocation
"#)
}

/// A user two steps below its supply (as in test_account_committed), ordering
/// 100 a day against an account holding 100 at factor 2.
fn lagged_model() -> String {
    r#"
[kalix]
start = 2020-01-01
end = 2020-01-05

[acc.g]
accounts = name, size, initial, debit_factor,
           a,    1000, 100,     2,

[node.dam]
type = storage
loc = 0, 0
initial_volume = 12000
dimensions = 0,  0,     0,  0,
             10, 12000, 10, 0,
             11, 12100, 10, 1000,
ds_1 = reach

[node.reach]
type = routing
loc = 0, 10
lag = 2
ds_1 = user

[node.user]
type = regulated_user
loc = 0, 20
order = 100
accounts = a
ds_1 = sink

[node.sink]
type = blackhole
loc = 0, 30

[outputs]
node.user.order
node.user.order_due
node.user.diversion
node.dam.ds_1
acc.a.committed
acc.a.closing_balance
acc.a.debits
"#.to_string()
}

// ------------------------------------------------------------------------------------------
// Contracts 1-2 on an unregulated user
// ------------------------------------------------------------------------------------------

#[test]
fn take_is_limited_to_balance_over_factor_and_charged_in_full() {
    let mut m = run(&unregulated_model("           a, 1000, 100, 2,", 80.0, "accounts = a", ""));
    assert_eq!(series(&mut m, "node.user.diversion"), vec![50.0, 0.0, 0.0], "100 at factor 2 is 50 of water");
    assert_eq!(series(&mut m, "acc.a.closing_balance"), vec![0.0, 0.0, 0.0]);
    assert_eq!(series(&mut m, "acc.a.debits"), vec![100.0, 0.0, 0.0], "debits record the amount charged");
    assert_eq!(series(&mut m, "acc.a.use"), vec![100.0, 100.0, 100.0]);
    assert_eq!(series(&mut m, "acc.a.allocation"), vec![100.0, 100.0, 100.0], "a take does not change the allocation");
}

#[test]
fn factor_one_is_the_plain_account() {
    let mut m = run(&unregulated_model("           a, 1000, 100, 1,", 80.0, "accounts = a", ""));
    assert_eq!(series(&mut m, "node.user.diversion"), vec![80.0, 20.0, 0.0]);
    assert_eq!(series(&mut m, "acc.a.debits"), vec![80.0, 20.0, 0.0]);
}

#[test]
fn plain_list_sums_water_and_charges_each_account_at_its_factor() {
    let rows = "           a, 1000, 100, 2,\n           b, 1000, 30,  1,";
    let mut m = run(&unregulated_model(rows, 80.0, "accounts = a, b", ""));
    assert_eq!(series(&mut m, "node.user.diversion"), vec![80.0, 0.0, 0.0], "50 from a, then 30 from b");
    assert_eq!(balance(&m, "a"), 0.0);
    assert_eq!(balance(&m, "b"), 0.0);
}

// ------------------------------------------------------------------------------------------
// Contract 4: combinations
// ------------------------------------------------------------------------------------------

#[test]
fn and_is_limited_by_the_smaller_water_and_charges_each_side_at_its_factor() {
    let rows = "           a, 1000, 100, 2,\n           b, 1000, 30,  1,";
    let mut m = run(&unregulated_model(rows, 80.0, "accounts = a & b", ""));
    assert_eq!(series(&mut m, "node.user.diversion"), vec![30.0, 0.0, 0.0], "a covers 50 of water, b 30");
    assert_eq!(balance(&m, "a"), 40.0, "the 30 taken charged 60 to a");
    assert_eq!(balance(&m, "b"), 0.0);
}

#[test]
fn pipe_matches_the_plain_list() {
    let rows = "           a, 1000, 100, 2,\n           b, 1000, 30,  1,";
    let mut as_list = run(&unregulated_model(rows, 70.0, "accounts = a, b", ""));
    let mut as_tree = run(&unregulated_model(rows, 70.0, "accounts = (a | b) & b_cap",
        "[acc.caps]\naccounts = name, size, initial,\n           b_cap, 1000, 1000,\n"));
    assert_eq!(series(&mut as_tree, "node.user.diversion"), series(&mut as_list, "node.user.diversion"));
    assert_eq!(series(&mut as_tree, "node.user.diversion"), vec![70.0, 10.0, 0.0]);
    assert_eq!(balance(&as_tree, "a"), balance(&as_list, "a"));
    assert_eq!(balance(&as_tree, "b"), balance(&as_list, "b"));
}

// ------------------------------------------------------------------------------------------
// Contract 1: roll_cap banks what was charged
// ------------------------------------------------------------------------------------------

#[test]
fn roll_cap_credits_back_the_charged_amount() {
    // roll_cap(1) every step credits back the previous step's debits at the top of the next
    let ras = "[ras.cap]\ntargets = acc.g\ntrigger = every_step\naction = roll_cap(1)\n";
    let mut m = run(&unregulated_model("           a, 100, 100, 2,", 20.0, "accounts = a", ras));
    assert_eq!(series(&mut m, "acc.a.closing_balance"), vec![60.0, 60.0, 60.0],
               "each 20 taken charges 40, and the 40 is credited back");
}

// ------------------------------------------------------------------------------------------
// Contracts 2-3 on a regulated user
// ------------------------------------------------------------------------------------------

#[test]
fn order_is_capped_in_water_and_committed_as_charged() {
    let mut m = run(&lagged_model());
    assert_eq!(series(&mut m, "node.user.order"), vec![50.0, 0.0, 0.0, 0.0, 0.0], "the node orders water");
    assert_eq!(series(&mut m, "node.dam.ds_1"), vec![50.0, 0.0, 0.0, 0.0, 0.0]);
    assert_eq!(series(&mut m, "node.user.order_due"), vec![0.0, 0.0, 50.0, 0.0, 0.0]);
    assert_eq!(series(&mut m, "node.user.diversion"), vec![0.0, 0.0, 50.0, 0.0, 0.0]);
    assert_eq!(series(&mut m, "acc.a.committed"), vec![0.0, 100.0, 100.0, 0.0, 0.0], "committed as charged");
    assert_eq!(series(&mut m, "acc.a.closing_balance"), vec![100.0, 100.0, 0.0, 0.0, 0.0]);
    assert_eq!(series(&mut m, "acc.a.debits"), vec![0.0, 0.0, 100.0, 0.0, 0.0]);
}

#[test]
fn order_account_excess_is_charged_at_its_factor() {
    let ini = r#"
[kalix]
start = 2020-01-01
end = 2020-01-02

[acc.g]
accounts = name, size, initial, debit_factor,
           a,    1000, 10,      1,
           oa,   1000, 100,     2,

[node.dam]
type = storage
loc = 0, 0
initial_volume = 500
dimensions = 0,  0,     0,  0,
             10, 12000, 10, 0,
             11, 12100, 10, 1000,
ds_1 = user

[node.user]
type = regulated_user
loc = 0, 10
order = 40
accounts = a
order_accounts = oa
ds_1 = sink

[node.sink]
type = blackhole
loc = 0, 20

[outputs]
node.user.order
acc.oa.debits
"#;
    let mut m = run(ini);
    assert_eq!(series(&mut m, "node.user.order")[0], 40.0, "10 from a, 30 authorised by oa");
    assert_eq!(series(&mut m, "acc.oa.debits")[0], 60.0, "the 30 excess charged 60 to oa");
}

// ------------------------------------------------------------------------------------------
// Contract 5: load
// ------------------------------------------------------------------------------------------

#[test]
fn factor_must_be_finite_normal_and_positive() {
    for bad in ["0", "-1", "NaN", "inf", "1e400", "1e-320"] {
        let ini = unregulated_model(&format!("           a, 1000, 100, {},", bad), 10.0, "accounts = a", "");
        let err = IniModelIO::read_model_string(&ini).err()
            .unwrap_or_else(|| panic!("debit_factor {} should be a load error", bad)).to_string();
        assert!(err.contains("must be a finite number greater than 0"), "{}: {}", bad, err);
    }
}

#[test]
fn reciprocal_is_set_at_load_and_survives_a_rerun() {
    let mut m = load(&unregulated_model("           a, 1000, 100, 4,", 10.0, "accounts = a", ""));
    let idx = m.account_manager.get_account_idx("a").unwrap();
    assert_eq!(m.account_manager.get_account(idx).unwrap().inv_debit_factor, 0.25);
    m.run().unwrap();
    let first = series(&mut m, "node.user.diversion");
    m.run().unwrap();
    assert_eq!(m.account_manager.get_account(idx).unwrap().inv_debit_factor, 0.25);
    assert_eq!(series(&mut m, "node.user.diversion"), first);
}

#[test]
fn a_full_draw_at_an_inexact_factor_leaves_no_meaningful_balance() {
    // 10 * (1/3) * 3 need not be exactly 10: the residue is a rounding step, never a take
    let mut m = run(&unregulated_model("           a, 1000, 10, 3,", 80.0, "accounts = a", ""));
    let diversion = series(&mut m, "node.user.diversion");
    assert!((diversion[0] - 10.0 / 3.0).abs() < 1e-12, "{:?}", diversion);
    assert!(diversion[1].abs() < 1e-12 && diversion[2].abs() < 1e-12, "{:?}", diversion);
    assert!(balance(&m, "a").abs() < 1e-12, "{}", balance(&m, "a"));
}
