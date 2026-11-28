//! Where the passwords are kept: the system's credential store in the app, a sealed file on
//! Klue Server.

use klue_core::store::{Account, Secrets, VaultAccount};

pub trait PasswordStore: Send + Sync {
    fn get(&self, account: &Account) -> Option<String>;
    /// `false` if it couldn't be saved (it's then only kept in memory).
    fn set(&self, account: &Account, password: &str) -> bool;
    fn delete(&self, account: &Account);
    /// Notes which numbers have passwords, so they can come back if the data file is lost.
    fn save_index(&self, accounts: &[Account]);
    fn load_index(&self) -> Vec<VaultAccount>;
}

impl PasswordStore for Secrets {
    fn get(&self, account: &Account) -> Option<String> {
        Secrets::get(self, account)
    }

    fn set(&self, account: &Account, password: &str) -> bool {
        Secrets::set(self, account, password)
    }

    fn delete(&self, account: &Account) {
        Secrets::delete(self, account)
    }

    fn save_index(&self, accounts: &[Account]) {
        Secrets::save_index(self, accounts)
    }

    fn load_index(&self) -> Vec<VaultAccount> {
        Secrets::load_index(self)
    }
}
