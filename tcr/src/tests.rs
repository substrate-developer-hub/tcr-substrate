use super::*;

use sp_core::H256;
use sp_runtime::{Perbill, traits::{BlakeTwo256, IdentityLookup}, testing::Header};
use frame_support::{impl_outer_origin, assert_ok, assert_noop, parameter_types, weights::Weight};

impl_outer_origin! {
	pub enum Origin for Test {}
}

// For testing the module, we construct most of a mock runtime. This means
// first constructing a configuration type (`Test`) which `impl`s each of the
// configuration traits of modules we want to use.
#[derive(Clone, Eq, PartialEq)]
pub struct Test;
parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const MaximumBlockWeight: Weight = 1024;
	pub const MaximumBlockLength: u32 = 2 * 1024;
	pub const AvailableBlockRatio: Perbill = Perbill::one();
}
impl system::Trait for Test {
	type Origin = Origin;
	type Index = u64;
	type Call = ();
	type BlockNumber = u64;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = ();
	type BlockHashCount = BlockHashCount;
	type MaximumBlockWeight = MaximumBlockWeight;
	type AvailableBlockRatio = AvailableBlockRatio;
	type MaximumBlockLength = MaximumBlockLength;
	type Version = ();
	type ModuleToIndex = ();
}
parameter_types! {
	pub const ExistentialDeposit: u64 = 500;
	pub const TransferFee: u64 = 0;
	pub const CreationFee: u64 = 0;
}
impl balances::Trait for Test {
	type Balance = u64;
	type OnFreeBalanceZero = ();
	type DustRemoval = ();
	type Event = ();
	type ExistentialDeposit = ExistentialDeposit;
	type TransferFee = TransferFee;
	type CreationFee = CreationFee;
	type OnNewAccount = ();
	type TransferPayment = ();
}
parameter_types! {
	pub const MinimumPeriod: u64 = 5;
}
impl Trait for Test {
	type Event = ();
	type ListingId = u32;
	type Currency = balances::Module<Self>;
}
type Tcr = Module<Test>;

// Builds the genesis config store and sets mock values.
fn new_test_ext() -> sp_io::TestExternalities {
	let mut t = system::GenesisConfig::default()
		.build_storage::<Test>()
		.unwrap();
	let _ = balances::GenesisConfig::<Test>{
		balances: vec![
			(1, 1000000),
			(2, 1000000),
			(3, 1000000),
			(4, 1000000),
		],
		vesting: vec![],
	}.assimilate_storage(&mut t).unwrap();

	GenesisConfig::<Test> {
		min_deposit: 100,
		apply_stage_len: 10,
		commit_stage_len: 10,
	}.assimilate_storage(&mut t).unwrap();

	t.into()
}

#[test]
fn should_fail_low_deposit() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			Tcr::propose(Origin::signed(1), 1, 99),
			"deposit should be more than min_deposit"
		);
	});
}

#[test]
fn should_pass_propose() {
	new_test_ext().execute_with(|| {
		assert_ok!(Tcr::propose(
			Origin::signed(1),
			1,
			101
		));
	});
}

#[test]
fn should_fail_challenge_same_owner() {
	new_test_ext().execute_with(|| {
		assert_ok!(Tcr::propose(
			Origin::signed(1),
			1,
			101
		));
		assert_noop!(
			Tcr::challenge(Origin::signed(1), 1, 101),
			"You cannot challenge your own listing."
		);
	});
}

#[test]
fn should_pass_challenge() {
	new_test_ext().execute_with(|| {
		assert_ok!(Tcr::propose(
			Origin::signed(1),
			1,
			101
		));
		assert_ok!(Tcr::challenge(Origin::signed(2), 1, 101));
	});
}

#[test]
fn cant_promote_too_early() {
	new_test_ext().execute_with(|| {
		assert_ok!(Tcr::propose(Origin::signed(1), 1, 101));
		assert_noop!(Tcr::promote_application(Origin::signed(1), 1),
		"Too early to promote this application.");
	});
}

#[test]
fn cant_promote_challenged_proposal() {
	new_test_ext().execute_with(|| {
		assert_ok!(Tcr::propose(Origin::signed(1), 1, 101));
		assert_ok!(Tcr::challenge(Origin::signed(2), 1, 300));
		assert_noop!(Tcr::promote_application(Origin::signed(1), 1),
		"Cannot promote a challenged listing.");
	});
}

#[test]
fn can_promote_unchallenged_proposal() {
	new_test_ext().execute_with(|| {

	});
}

#[test]
fn aye_vote_works_correctly() {
	new_test_ext().execute_with(|| {

	});
}

#[test]
fn nay_vote_works_correctly() {
	new_test_ext().execute_with(|| {

	});
}

#[test]
fn successfully_challenged_listings_are_removed() {
	new_test_ext().execute_with(|| {

	});
}

#[test]
fn unsuccessfully_challenged_listings_are_kept() {
	new_test_ext().execute_with(|| {

	});
}