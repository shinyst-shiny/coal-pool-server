use super::AccountDiscriminator;
use bytemuck::{Pod, Zeroable};
use steel::*;

/// Treasury is a singleton account which is the mint authority for the ORE token and the authority of
/// the program's global token account.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct Treasury {}

/*impl_to_bytes!(Treasury);
impl_account_from_bytes!(Treasury);
*/
account!(AccountDiscriminator, Treasury);
