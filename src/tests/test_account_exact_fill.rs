// A take or order that reaches its accounts' cap empties them exactly.
//
// The cap is the sum of each account's limit in water (balance / debit_factor). Walking it
// back down by subtraction, or summing a take split into parts, lands a rounding step short
// of an account's limit, which left about 1e-14 in the account and a phantom take or order of
// it the next step. The balances below are ones that did, at factor 1 as well as others.
//
// The contract:
// 1. PLAIN LIST: at the cap every account is drained (charged exactly its balance).
// 2. COMBINATION: a `|` at its capacity drains every child, also as a side of `&` and with an
//    `&` among its children.
// 3. SPLIT TAKES: a take split into outlets, own and opportunistic parts, or an order split
//    between the accounts and the order_accounts, drains them when it reaches the cap.
// 4. NO OVERDRAW: just under the cap, no account goes below zero.
use crate::hydrology::accounts::account::Account;
use crate::hydrology::accounts::account_manager::AccountManager;
use crate::io::ini_model_io::IniModelIO;
use crate::model::Model;

fn run(ini: &str) -> Model {
    let mut model = IniModelIO::read_model_string(ini).expect("model should load");
    model.configure().expect("model should configure");
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

/// inflow (500) -> unregulated user -> blackhole, two days, demand far above the accounts
fn unregulated_model(acc_rows: &str, accounts_line: &str) -> String {
    format!(r#"
[kalix]
start = 2020-01-01
end = 2020-01-02

[acc.g]
accounts = name, size, initial, debit_factor,
{acc_rows}

[node.src]
type = inflow
loc = 0, 0
inflow = 500
ds_1 = user

[node.user]
type = unregulated_user
loc = 0, 10
demand = 400
{accounts_line}
ds_1 = sink

[node.sink]
type = blackhole
loc = 0, 20

[outputs]
node.user.diversion
"#)
}

fn assert_drained(m: &mut Model, accounts: &[&str]) {
    for name in accounts {
        assert_eq!(balance(m, name), 0.0, "'{}' is drained exactly", name);
    }
    assert_eq!(series(m, "node.user.diversion")[1], 0.0, "nothing left to take on day 2");
}

// ------------------------------------------------------------------------------------------
// Contract 1: plain list
// ------------------------------------------------------------------------------------------

#[test]
fn a_list_at_the_cap_drains_every_account() {
    let rows = "           a, 1000, 100, 1.2,\n           b, 1000, 80,  1.3,";
    let mut m = run(&unregulated_model(rows, "accounts = a, b"));
    assert_drained(&mut m, &["a", "b"]);
}

#[test]
fn a_list_at_the_cap_drains_every_account_at_factor_one() {
    // 0.7 + 0.1 - 0.7 is a rounding step under 0.1
    let rows = "           a, 1000, 0.7, 1,\n           b, 1000, 0.1, 1,";
    let mut m = run(&unregulated_model(rows, "accounts = a, b"));
    assert_drained(&mut m, &["a", "b"]);
}

// ------------------------------------------------------------------------------------------
// Contract 2: combinations
// ------------------------------------------------------------------------------------------

#[test]
fn a_pipe_binding_an_and_drains_every_account() {
    let rows = "           a,   1000,  100,   1.2,\n           b,   1000,  80,    1.3,\n           cap, 20000, 10000, 1,";
    let mut m = run(&unregulated_model(rows, "accounts = (a | b) & cap"));
    assert_drained(&mut m, &["a", "b"]);
}

#[test]
fn an_and_inside_a_pipe_drains_its_binding_side() {
    let rows = "           a,   1000,  100,   1.2,\n           b,   1000,  80,    1.3,\n           cap, 20000, 10000, 1,";
    let mut m = run(&unregulated_model(rows, "accounts = a | (b & cap)"));
    assert_drained(&mut m, &["a", "b"]);
}

// ------------------------------------------------------------------------------------------
// Contract 3: split takes and orders
// ------------------------------------------------------------------------------------------

#[test]
fn a_regulated_and_opportunistic_take_at_the_cap_drains_the_account() {
    // 0.1 regulated, the rest of 0.3 / 0.7 opportunistic: 0.1 + (d - 0.1) is a step under d.
    // The dam spills, so there is river water for the opportunistic take.
    let ini = r#"
[kalix]
start = 2020-01-01
end = 2020-01-02

[acc.g]
accounts = name, size, initial, debit_factor,
           a,    1000, 0.3,     0.7,

[node.dam]
type = storage
loc = 0, 0
initial_volume = 12100
dimensions = 0,  0,     0,  0,
             10, 12000, 10, 0,
             11, 12100, 10, 1000,
ds_1 = user

[node.user]
type = regulated_user
loc = 0, 10
order = 0.1
opportunistic_demand = 400
accounts = a
ds_1 = sink

[node.sink]
type = blackhole
loc = 0, 20

[outputs]
node.user.diversion
"#;
    let mut m = run(ini);
    assert_drained(&mut m, &["a"]);
}

#[test]
fn an_order_at_the_cap_drains_the_order_accounts() {
    // a covers 100 / 1.1 and oa 80 / 1.2: (r + o) - r is a step under o
    let ini = r#"
[kalix]
start = 2020-01-01
end = 2020-01-02

[acc.g]
accounts = name, size, initial, debit_factor,
           a,    1000, 100,     1.1,
           oa,   1000, 80,      1.2,

[node.dam]
type = storage
loc = 0, 0
initial_volume = 12000
dimensions = 0,  0,     0,  0,
             10, 12000, 10, 0,
             11, 12100, 10, 1000,
ds_1 = user

[node.user]
type = regulated_user
loc = 0, 10
order = 1000
accounts = a
order_accounts = oa
ds_1 = sink

[node.sink]
type = blackhole
loc = 0, 20

[outputs]
node.user.order
node.user.diversion
"#;
    let mut m = run(ini);
    assert_eq!(balance(&m, "oa"), 0.0, "the order_accounts are drained at order time");
    assert_eq!(balance(&m, "a"), 0.0, "the delivery drains the regular account (one account: always exact)");
    assert_eq!(series(&mut m, "node.user.order")[1], 0.0, "nothing left to order with on day 2");
}

#[test]
fn an_order_bound_by_an_and_leaf_commits_exactly_its_headroom() {
    // (a | b) & cap with cap binding at an inexact factor. One step of travel time, so the
    // commitment is published on day 2, before it is released.
    let ini = r#"
[kalix]
start = 2020-01-01
end = 2020-01-03

[acc.g]
accounts = name, size, initial, debit_factor,
           a,    20000, 10000,  1,
           b,    20000, 10000,  1,
           cap,  1000,  100,    1.2,

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
lag = 1
ds_1 = user

[node.user]
type = regulated_user
loc = 0, 20
order = 1000
accounts = (a | b) & cap
ds_1 = sink

[node.sink]
type = blackhole
loc = 0, 30

[outputs]
acc.cap.committed
"#;
    let mut m = run(ini);
    assert_eq!(series(&mut m, "acc.cap.committed")[1], 100.0, "exactly the headroom, nothing en route before it");
}

#[test]
fn a_list_order_at_the_cap_commits_exactly_each_headroom() {
    // One step of travel time, so the commitments are published on day 2, before release
    let ini = r#"
[kalix]
start = 2020-01-01
end = 2020-01-03

[acc.g]
accounts = name, size, initial, debit_factor,
           a,    1000, 100,     1.2,
           b,    1000, 80,      1.3,

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
lag = 1
ds_1 = user

[node.user]
type = regulated_user
loc = 0, 20
order = 1000
accounts = a, b
ds_1 = sink

[node.sink]
type = blackhole
loc = 0, 30

[outputs]
acc.a.committed
acc.b.committed
"#;
    let mut m = run(ini);
    assert_eq!(series(&mut m, "acc.a.committed")[1], 100.0);
    assert_eq!(series(&mut m, "acc.b.committed")[1], 80.0, "the last account's whole headroom");
}

#[test]
fn a_carried_over_demand_at_the_cap_drains_every_account() {
    // The demand beyond the accounts is carried over; the accounts are still emptied exactly
    let rows = "           a, 1000, 100, 1.2,\n           b, 1000, 80,  1.3,";
    let ini = unregulated_model(rows, "accounts = a, b").replace("demand = 400", "demand = 400\ndemand_carryover = true");
    let mut m = run(&ini);
    assert_drained(&mut m, &["a", "b"]);
}

// ------------------------------------------------------------------------------------------
// The bound tests drain only at the cap
// ------------------------------------------------------------------------------------------

#[test]
fn an_order_within_the_regular_accounts_leaves_the_order_accounts_alone() {
    let ini = r#"
[kalix]
start = 2020-01-01
end = 2020-01-01

[acc.g]
accounts = name, size, initial, debit_factor,
           a,    1000, 100,     1.1,
           oa,   1000, 80,      1.2,

[node.dam]
type = storage
loc = 0, 0
initial_volume = 12000
dimensions = 0,  0,     0,  0,
             10, 12000, 10, 0,
             11, 12100, 10, 1000,
ds_1 = user

[node.user]
type = regulated_user
loc = 0, 10
order = 30
accounts = a
order_accounts = oa
ds_1 = sink

[node.sink]
type = blackhole
loc = 0, 20
"#;
    let m = run(ini);
    assert_eq!(balance(&m, "oa"), 80.0);
    assert_eq!(balance(&m, "a"), 100.0 - 30.0 * 1.1);
}

#[test]
fn a_pump_limited_take_below_the_cap_is_charged_what_it_took() {
    let rows = "           a, 1000, 100, 1.2,\n           b, 1000, 80,  1.3,";
    let ini = unregulated_model(rows, "accounts = a, b").replace("demand = 400", "demand = 400\npump = 10");
    let mut m = run(&ini);
    assert_eq!(series(&mut m, "node.user.diversion"), vec![10.0, 10.0]);
    assert_eq!(balance(&m, "a"), 100.0 - 10.0 * 1.2 - 10.0 * 1.2);
    assert_eq!(balance(&m, "b"), 80.0, "b untouched");
}

#[test]
fn an_overdrawn_account_in_a_list_is_not_credited() {
    let mut manager = AccountManager::new();
    let a = manager.add_account(Account::new("a".into(), "g".into(), 1000.0, 0.0, 1.0)).unwrap();
    let b = manager.add_account(Account::new("b".into(), "g".into(), 1000.0, 50.0, 1.0)).unwrap();
    manager.set_account_balance_fast(a, -5.0);
    let cap = 0.0 + manager.get_account_debitable(a) + manager.get_account_debitable(b);
    assert_eq!(cap, 50.0);
    manager.take_in_order(&[a, b], cap, cap);
    assert_eq!(manager.get_account_balance(a), -5.0);
    assert_eq!(manager.get_account_balance(b), 0.0);
}

#[test]
fn below_the_cap_a_list_at_factor_one_is_walked_as_before() {
    // The balances the subtraction walk gave before the exact fill: 0.93 from 0.7, 0.1, 0.3
    let mut manager = AccountManager::new();
    let idxs: Vec<usize> = [("a", 0.7), ("b", 0.1), ("c", 0.3)].iter()
        .map(|&(name, b)| manager.add_account(Account::new(name.into(), "g".into(), 1000.0, b, 1.0)).unwrap())
        .collect();
    let cap = idxs.iter().map(|&idx| manager.get_account_debitable(idx)).sum();
    manager.take_in_order(&idxs, 0.93, cap);
    let balances: Vec<f64> = idxs.iter().map(|&idx| manager.get_account_balance(idx)).collect();
    assert_eq!(balances, vec![0.0, 0.0, 0.1699999999999999]);
}

// ------------------------------------------------------------------------------------------
// Contract 4: just under the cap
// ------------------------------------------------------------------------------------------

#[test]
fn a_take_a_step_under_the_cap_never_overdraws() {
    let factors = [1.0, 1.1, 1.2, 1.3, 0.7, 0.9, 3.0];
    let balances = [100.0, 80.0, 0.3, 0.7, 12.34, 387.996];
    for &fa in &factors {
        for &fb in &factors {
            for &ba in &balances {
                for &bb in &balances {
                    let mut manager = AccountManager::new();
                    let a = manager.add_account(Account::new("a".into(), "g".into(), 1000.0, ba, fa)).unwrap();
                    let b = manager.add_account(Account::new("b".into(), "g".into(), 1000.0, bb, fb)).unwrap();
                    let cap = 0.0 + manager.get_account_debitable(a) + manager.get_account_debitable(b);
                    manager.take_in_order(&[a, b], cap.next_down(), cap);
                    for idx in [a, b] {
                        assert!(manager.get_account_balance(idx) >= 0.0,
                            "a {}@{}, b {}@{}: account {} overdrawn to {}", ba, fa, bb, fb, idx,
                            manager.get_account_balance(idx));
                    }
                }
            }
        }
    }
}
