use crate::numerical::fifo_buffer::FifoBuffer;

/// A single account: pure state — name, cap, balance. Accounts carry no
/// behaviour and no calendar; everything that changes a balance is a [ras.*]
/// action or a node take (kalix-allocation-components.md §3.1).
// No Default: an account is built only by `new`, which sets the factor and its reciprocal together
#[derive(Clone)]
pub struct Account {
    // Properties
    pub name: String,
    /// The declaring group's name (the acc. addressing/targeting unit).
    pub account_type: String,
    pub size: f64,
    pub initial_balance: f64,
    /// Paired account (the `pair` table column), resolved to its index at
    /// load. The pairing is symmetric — declared on either account's row,
    /// readable from both ends as `self.pair.<field>` in [ras.*] action
    /// arguments — and an account may be in at most one pair. The one
    /// declared relationship an account carries: accounts stay pure state
    /// otherwise — debit order remains the user node's `accounts =` list,
    /// never baked in here.
    pub pair: Option<usize>,
    /// Whether this row is the one that declared the pairing — round-trip
    /// only, so the canonical render emits the `pair` column on the side
    /// that wrote it and not both.
    pub pair_declared: bool,

    /// Debit factor (the `debit_factor` column, > 0, default 1.0): a take of
    /// x charges x * debit_factor to the account. Balance, committed and the
    /// debit tallies all hold the charged amount; the node's take stays water.
    pub(super) debit_factor: f64,
    /// 1 / debit_factor, precomputed at load so the take-side reads multiply
    /// rather than divide. Exactly 1.0 when the factor is 1.0.
    pub(super) inv_debit_factor: f64,

    // State
    pub balance: f64,
    /// Debits from node takes so far this step. Reset at the top of every
    /// step; only `charge` (a node's take) adds to it, so policy changes made by
    /// [ras.*] actions (write-offs, resets, credits) are excluded by
    /// construction — this is what takes charged (water used times the debit
    /// factor), not "balance moved".
    pub debits_today: f64,

    /// Debits from node takes since the last allocation reset. Together with
    /// the balance this *is* the account's allocation to date: a take does not
    /// reduce an allocation, its charge moves between the two terms. Reset only by
    /// the `reset_allocation` action (kalix-allocation-components.md §3.4).
    pub debits_since_reset: f64,

    /// Debits from node takes since the last `roll_cap` action.
    pub debits_this_period: f64,
    /// Completed-period debit totals not yet expired by `roll_cap` (N-1
    /// slots). Zero-capacity (passthrough) unless roll_cap targets this
    /// account with N > 1.
    pub aged_debits: FifoBuffer,

    /// Orders accepted against this account and not yet due: the users' best
    /// guess of the diversions to come. A forecast, not a debit - it never
    /// moves the balance, and `balance - committed` is what a regulated user
    /// may still order. Only the actual diversion is ever debited.
    ///
    /// Kept exact by `commit_ring`, a calendar of the committed amounts by the
    /// step they fall due: slot `commit_cursor` is today, slot `cursor + k` is
    /// k steps ahead. A user with travel time T commits into slot `cursor + T`;
    /// at the start of every ordering phase the account releases yesterday's
    /// slot - the orders that fell due yesterday, delivered or not - and the
    /// cursor moves on to today (AccountManager::start_of_ordering). Every
    /// amount so leaves the account it entered on the step its order is due,
    /// whichever user placed it and whatever its travel time, with no record
    /// of which order was whose. `committed` is the sum of the ring. The ring
    /// is sized by the ordering system (longest travel time among the users
    /// drawing on the account, plus one) and is empty for an account no
    /// regulated user draws on.
    pub committed: f64,
    pub commit_ring: Vec<f64>,
    pub commit_cursor: usize,
}

impl Account {

    // Constructor
    pub fn new(name: String, account_type: String, size: f64, initial_balance: f64, debit_factor: f64) -> Self {
        // The reader enforces this; the take paths rely on 1 / f being finite and non-zero
        debug_assert!(debit_factor.is_normal() && debit_factor > 0.0, "debit_factor {} must be finite, normal and > 0", debit_factor);
        Account {
            name,
            account_type,
            size,
            initial_balance,
            pair: None,
            pair_declared: false,
            debit_factor,
            inv_debit_factor: 1.0 / debit_factor,
            balance: initial_balance,
            debits_today: 0.0,
            debits_since_reset: 0.0,
            debits_this_period: 0.0,
            aged_debits: FifoBuffer::default(),
            committed: 0.0,
            commit_ring: Vec::new(),
            commit_cursor: 0,
        }
    }

    /// What a take of one unit of water charges this account.
    pub fn debit_factor(&self) -> f64 {
        self.debit_factor
    }

    // Initialize account using saved initial balance
    pub fn initialize(&mut self) {
        self.balance = self.initial_balance;
        self.debits_today = 0.0;
        self.debits_since_reset = 0.0;
        self.debits_this_period = 0.0;
        self.aged_debits.reset();
        self.committed = 0.0;
        self.commit_ring.fill(0.0);
        self.commit_cursor = 0;
    }

    /// Allocation to date: everything credited since the last reset, whether
    /// it is still held or has been used. This is the quantity an announced
    /// allocation percentage sets, and the reason a user drawing their balance
    /// down does not reduce their allocation.
    pub fn allocation(&self) -> f64 {
        self.balance + self.debits_since_reset
    }

    // Set balance but not allowing it to be less than 0 or greater
    // than the account size.
    pub fn set_balance_safely(&mut self, balance: f64) {
        self.balance = balance.max(0.0).min(self.size);
    }

    // Sets balance without checking the validity.
    pub fn set_balance_fast(&mut self, balance: f64) {
        self.balance = balance
    }

    // Add some value (positive or negative), but not allowing the
    // new value to become less than 0 or greater than the account
    // size.
    pub fn add_value_safely(&mut self, amount: f64) {
        let temp_bal = self.balance + amount;
        self.set_balance_safely(temp_bal);
    }

    // Add some value to the account balance without checking the validity
    // of the resulting account balance.
    pub fn credit_account_fast(&mut self, amount: f64) {
        self.balance += amount;
    }

    // Subtract some value from the account balance without checking the validity
    // of the resulting account balance.
    pub fn debit_account_fast(&mut self, amount: f64) {
        self.balance -= amount;
    }

    // Sets the account balance to the given fraction of the account size
    pub fn set_balance_fraction(&mut self, balance_as_proportion_of_account_size: f64) {
        self.set_balance_fast(balance_as_proportion_of_account_size * self.size);
    }

    /// Charge a node's take to the account: the balance falls by `charged`
    /// and the debit tallies rise by it.
    #[inline]
    pub(super) fn charge(&mut self, charged: f64) {
        self.debit_account_fast(charged);
        self.debits_today += charged;
        self.debits_since_reset += charged;
        self.debits_this_period += charged;
    }

    /// Add `charged` to the orders committed against the account, due in
    /// `due_in` steps. The ring must have been sized for this user by
    /// `set_commit_horizon`, so `due_in` is within it, and `start_of_ordering`
    /// must have moved the cursor on to today.
    #[inline]
    pub(super) fn add_commitment(&mut self, charged: f64, due_in: usize) {
        debug_assert!(due_in < self.commit_ring.len(), "commit ring of '{}' not sized for a travel time of {}", self.name, due_in);
        let mut slot = self.commit_cursor + due_in;
        if slot >= self.commit_ring.len() { slot -= self.commit_ring.len(); }
        self.commit_ring[slot] += charged;
        self.committed += charged;
    }
}
