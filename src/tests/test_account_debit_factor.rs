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
// 5. LOAD: the factor must be a finite, normal number greater than 0. 1 / f is
//    precomputed at load and survives a rerun.
// 6. EXACT AT THE LIMIT: a take or order that uses all an account allows charges
//    or commits exactly what was left, however (balance / f) * f rounds.
use crate::io::ini_model_io::IniModelIO;
use crate::misc::configuration::SaveMethod;
use crate::model::Model;

fn load(ini: &str) -> Model {
    let mut model = IniModelIO::read_model_string(ini).expect("model should load");
    model.configure().expect("model should configure");
    model
}

fn run(ini: &str) -> Model {
    let mut model = load(ini);
    model.run().expect("simulation should run");
    model
}

fn series(model: &mut Model, name: &str) -> Vec<f64> {
    let idx = model.data_cache.get_series_idx(name, false)
        .unwrap_or_else(|| panic!("no series '{}'", name));
    model.data_cache.series[idx].values.clone()
}

fn balance(model: &Model, name: &str) -> f64 {
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
/// 100 a day against an account holding 100 at the given factor.
fn lagged_model(factor: f64) -> String {
    format!(r#"
[kalix]
start = 2020-01-01
end = 2020-01-05

[acc.g]
accounts = name, size, initial, debit_factor,
           a,    1000, 100,     {factor},

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
"#)
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
    let mut m = run(&lagged_model(2.0));
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
    assert_eq!(m.account_manager.get_account_debitable(idx), 25.0, "100 over 4");
    m.run().unwrap();
    let first = series(&mut m, "node.user.diversion");
    assert_eq!(first, vec![10.0, 10.0, 5.0], "10 charges 40 a day, so the third day has 20 / 4");
    m.run().unwrap();
    assert_eq!(series(&mut m, "node.user.diversion"), first);
}

#[test]
fn a_full_draw_at_an_inexact_factor_charges_exactly_the_balance() {
    // (100 / f) * f is not 100 at these factors: 3 leaves dust, 1.2 overdraws
    for factor in ["3", "1.2"] {
        let rows = format!("           a, 1000, 100, {},", factor);
        let mut m = run(&unregulated_model(&rows, 200.0, "accounts = a", ""));
        let diversion = series(&mut m, "node.user.diversion");
        assert_eq!(&diversion[1..], &[0.0, 0.0], "factor {}: no dust left to take", factor);
        assert_eq!(series(&mut m, "acc.a.closing_balance"), vec![0.0, 0.0, 0.0], "factor {}", factor);
        assert_eq!(series(&mut m, "acc.a.debits"), vec![100.0, 0.0, 0.0], "factor {}", factor);
    }
}

#[test]
fn a_full_order_at_an_inexact_factor_commits_exactly_the_headroom() {
    let mut m = run(&lagged_model(3.0));
    let order = series(&mut m, "node.user.order");
    assert_eq!(&order[1..], &[0.0, 0.0, 0.0, 0.0], "no phantom order from uncommitted dust");
    assert_eq!(series(&mut m, "acc.a.committed"), vec![0.0, 100.0, 100.0, 0.0, 0.0]);
    assert_eq!(series(&mut m, "acc.a.closing_balance"), vec![100.0, 100.0, 0.0, 0.0, 0.0]);
    assert_eq!(series(&mut m, "acc.a.debits"), vec![0.0, 0.0, 100.0, 0.0, 0.0]);
}

#[test]
fn canonical_save_round_trips_the_factor_exactly() {
    let ini = r#"
[kalix]
start = 2020-01-01
end = 2020-01-02

[acc.g]
accounts = name, size, initial, pair, debit_factor,
           a,    1000, 100,     c,    1.1764705882352942,
           b,    1000, 50,      d,    1.00000000001,

[acc.h]
accounts = name, size, initial, debit_factor,
           c,    500,  0,       1,
           d,    500,  0,       1,

[node.src]
type = inflow
loc = 0, 0
inflow = 500
ds_1 = sink

[node.sink]
type = blackhole
loc = 0, 10
"#;
    let mut model = IniModelIO::read_model_string(ini).expect("model should load");
    model.configuration.save_method = SaveMethod::Canonical;
    let saved = IniModelIO::model_to_string(&model);
    assert!(saved.contains("accounts = name, size, initial, pair, debit_factor,"), "pair before debit_factor:
{}", saved);
    assert_eq!(saved.matches("debit_factor").count(), 1, "an all-1 group drops the column:
{}", saved);
    let reloaded = IniModelIO::read_model_string(&saved)
        .unwrap_or_else(|e| panic!("saved model should reload: {}
{}", e, saved));
    let am = &reloaded.account_manager;
    let account = |name: &str| am.get_account(am.get_account_idx(name).unwrap()).unwrap();
    assert_eq!(account("a").debit_factor, 1.1764705882352942);
    assert_eq!(account("b").debit_factor, 1.00000000001, "not rounded to 1");
    assert_eq!(account("c").debit_factor, 1.0);
    assert_eq!(account("a").pair, am.get_account_idx("c"), "pairing survives");
}

/// dam -> reach (lag) -> regulated user -> blackhole, five days
fn regulated_model(acc_rows: &str, lag: u32, user_lines: &str, outputs: &str) -> String {
    format!(r#"
[kalix]
start = 2020-01-01
end = 2020-01-05

[acc.g]
accounts = name, size, initial, debit_factor,
{acc_rows}

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
lag = {lag}
ds_1 = user

[node.user]
type = regulated_user
loc = 0, 20
{user_lines}
ds_1 = sink

[node.sink]
type = blackhole
loc = 0, 30

[outputs]
node.user.order
node.user.diversion
{outputs}
"#)
}

#[test]
fn combination_order_is_capped_committed_and_charged_per_side() {
    // (a | b) can cover 50 + 30 = 80 of water, cap 2000: the order is 80
    let rows = "           a,   1000, 100,  2,
           b,   1000, 30,   1,
           cap, 1000, 1000, 0.5,";
    let mut m = run(&regulated_model(rows, 2, "order = 100
accounts = (a | b) & cap",
        "acc.a.committed
acc.b.committed
acc.cap.committed
acc.a.debits
acc.b.debits
acc.cap.debits"));
    assert_eq!(series(&mut m, "node.user.order"), vec![80.0, 0.0, 0.0, 0.0, 0.0]);
    assert_eq!(series(&mut m, "acc.a.committed"), vec![0.0, 100.0, 100.0, 0.0, 0.0], "a's 50 of water committed as 100");
    assert_eq!(series(&mut m, "acc.b.committed"), vec![0.0, 30.0, 30.0, 0.0, 0.0]);
    assert_eq!(series(&mut m, "acc.cap.committed"), vec![0.0, 40.0, 40.0, 0.0, 0.0], "the whole 80 at 0.5");
    assert_eq!(series(&mut m, "node.user.diversion"), vec![0.0, 0.0, 80.0, 0.0, 0.0]);
    assert_eq!(series(&mut m, "acc.a.debits"), vec![0.0, 0.0, 100.0, 0.0, 0.0]);
    assert_eq!(series(&mut m, "acc.b.debits"), vec![0.0, 0.0, 30.0, 0.0, 0.0]);
    assert_eq!(series(&mut m, "acc.cap.debits"), vec![0.0, 0.0, 40.0, 0.0, 0.0]);
    assert_eq!(balance(&m, "cap"), 960.0);
}

#[test]
fn combination_overflow_and_order_accounts_are_charged_at_their_factors() {
    // (a | b) covers 80 of water and oa 100 more, so an order of 150 is accepted. The 70 the
    // left side cannot cover goes on its first account, a, at a's factor; oa is debited the
    // 70 excess at its own factor.
    let rows = "           a,   1000, 100,  2,
           b,   1000, 30,   1,
           cap, 1000, 1000, 0.5,
           oa,  1000, 400,  4,";
    let mut m = run(&regulated_model(rows, 2, "order = 150
accounts = (a | b) & cap
order_accounts = oa",
        "acc.a.committed
acc.b.committed
acc.cap.committed
acc.oa.debits"));
    assert_eq!(series(&mut m, "node.user.order")[0], 150.0);
    assert_eq!(series(&mut m, "acc.a.committed")[1], 240.0, "50 of water at the limit (100) plus the 70 overflow at 2");
    assert_eq!(series(&mut m, "acc.b.committed")[1], 30.0);
    assert_eq!(series(&mut m, "acc.cap.committed")[1], 75.0, "the whole 150 at 0.5");
    assert_eq!(series(&mut m, "acc.oa.debits")[0], 280.0, "the 70 excess at 4");
}

#[test]
fn order_accounts_extend_the_order_cap_in_water() {
    // a covers 10 of water and oa 100 / 2 = 50, so an order of 200 is cut to 60
    let rows = "           a,  1000, 10,  1,
           oa, 1000, 100, 2,";
    let mut m = run(&regulated_model(rows, 0, "order = 200
accounts = a
order_accounts = oa", "acc.oa.debits"));
    assert_eq!(series(&mut m, "node.user.order")[0], 60.0);
    assert_eq!(series(&mut m, "acc.oa.debits")[0], 100.0, "oa's whole balance, exactly");
    assert_eq!(balance(&m, "oa"), 0.0);
}

#[test]
fn opportunistic_take_is_capped_by_balance_over_factor() {
    // River flow, no order: the opportunistic take is the only take
    let ini = r#"
[kalix]
start = 2020-01-01
end = 2020-01-03

[acc.g]
accounts = name, size, initial, debit_factor,
           a,    1000, 100,     2,

[node.src]
type = inflow
loc = 0, 0
inflow = 500
ds_1 = user

[node.user]
type = regulated_user
loc = 0, 10
order = 0
opportunistic_demand = 80
accounts = a
ds_1 = sink

[node.sink]
type = blackhole
loc = 0, 20

[outputs]
node.user.diversion_opportunistic
acc.a.debits
"#;
    let mut m = run(ini);
    assert_eq!(series(&mut m, "node.user.diversion_opportunistic"), vec![50.0, 0.0, 0.0]);
    assert_eq!(series(&mut m, "acc.a.debits"), vec![100.0, 0.0, 0.0]);
    assert_eq!(balance(&m, "a"), 0.0);
}
