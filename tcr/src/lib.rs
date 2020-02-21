
#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use sp_std::prelude::*;
use sp_runtime::traits::{CheckedAdd, CheckedDiv, CheckedMul, Hash};
use frame_support::{
  decl_event, decl_module, decl_storage, dispatch::DispatchResult, print, ensure,
  traits::{ Currency, ReservableCurrency },
};
use system::{ensure_signed, ensure_root};

// Read TCR concepts here:
// https://www.gautamdhameja.com/token-curated-registries-explain-eli5-a5d4cce0ddbe/

// The module trait
pub trait Trait: system::Trait {
  type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
  type Currency: Currency<Self::AccountId> + ReservableCurrency<Self::AccountId>;
  // type ListingId: Hash + Encode + Decode + EncodeLike; //TODO What fucking trait bounds do I need to make this thing a storage map key
}

type ListingId = u32; //TODO figure out how to put this in the configuration trait.

type ChallengeId = u32;
type BalanceOf<T> = <<T as Trait>::Currency as Currency<<T as system::Trait>::AccountId>>::Balance;
type AccountIdOf<T> = <T as system::Trait>::AccountId;
type BlockNumberOf<T> = <T as system::Trait>::BlockNumber;
type ListingDetailOf<T> = ListingDetail<BalanceOf<T>, AccountIdOf<T>, BlockNumberOf<T>>;
type ChallengeDetailOf<T> = ChallengeDetail<<T as Trait>::ListingId, BalanceOf<T>, AccountIdOf<T>, BlockNumberOf<T>, VoteOf<T>>;
type VoteOf<T> = Vote<AccountIdOf<T>, BalanceOf<T>>;

#[cfg_attr(feature = "std", derive(Debug))]
#[derive(Encode, Decode, Default, Clone, PartialEq)]
pub struct ListingDetail<Balance, AccountId, BlockNumber> {
  deposit: Balance,
  owner: AccountId,
  application_expiry: BlockNumber,
  in_registry: bool,
  challenge_id: ChallengeId,
}

#[cfg_attr(feature = "std", derive(Debug))]
#[derive(Encode, Decode, Default, Clone, PartialEq)]
pub struct ChallengeDetail<ListingId, Balance, AccountId, BlockNumber, Vote> {
  listing_id: ListingId,
  deposit: Balance,
  owner: AccountId,
  voting_ends: BlockNumber,
  votes: Vec<Vote>,
}

#[cfg_attr(feature = "std", derive(Debug))]
#[derive(Encode, Decode, Default, Clone, PartialEq)]
pub struct Vote<AccountId, Balance> {
  voter: AccountId,
  aye_or_nay: bool, // true means: I want this item in the registry. false means: I do not want this item in the registry
  deposit: Balance,
}


decl_storage! {
  trait Store for Module<T: Trait> as Tcr {
    /// TCR parameter - minimum deposit.
    MinDeposit get(min_deposit) config(): Option<BalanceOf<T>>;

    /// TCR parameter - apply stage length - deadline for challenging before a listing gets accepted.
    ApplyStageLen get(apply_stage_len) config(): Option<T::BlockNumber>;

    /// TCR parameter - commit stage length - deadline for voting before a challenge gets resolved.
    CommitStageLen get(commit_stage_len) config(): Option<T::BlockNumber>;
    

    /// All listings and applicants known to the TCR. Inclusion in this map is NOT the same as listing in the registry,
    /// because this map also includes new applicants (some of which are challenged)
    Listings get(listings): map ListingId => ListingDetailOf<T>;

    /// The first unused challenge Id. Will become the Id of the next challenge when it is open.
    NextChallengeId get(next_challenge_id): ChallengeId;

    /// All currently open challenges
    Challenges get(challenges): map ChallengeId => ChallengeDetailOf<T>;

    /// Mapping from the blocknumber when a challenge expires to its challenge Id. This is used to 
    /// automatically resolve challenges in `on_finalize`. This storage item could be omitted if
    /// settling challenges were a mnaully triggered process.
    ChallengeExpiry get(challenge_expiry): map BlockNumberOf<T> => ChallengeId;
  }
}

// Events
decl_event!(
  pub enum Event<T>
  	where AccountId = <T as system::Trait>::AccountId,
	  Balance = BalanceOf<T>,
  	// ListingId = ListingIdOf<T>
  {
    /// A user has proposed a new listing
    Proposed(AccountId, ListingId, Balance),

    /// A user has challenged a listing. The challenged listing may be already listed,
    /// or an applicant
    Challenged(AccountId, ListingId, Balance),

    /// A user cast a vote in an already-existing challenge
    Voted(AccountId, ListingId, bool, Balance),

    /// A challenge has been resolved and the challenged listing included or excluded from the registry.
    /// This does not guarantee that the status of the challenged listing in the registry has changed.
    /// For example, a previously-listed item may have passed the challenge, or a new applicant may have
    /// failed the challenge.
    Resolved(ListingId, bool),

    /// A new, previously un-registered listing has been added to the Registry
    Accepted(ListingId),

    /// 
    Rejected(Hash),
    // When a vote reward is claimed for a challenge.
    Claimed(AccountId, u32),

    //TODO MAybe the last few events should be Added, Removed, Rejected, Defended
  }
);


decl_module! {
  pub struct Module<T: Trait> for enum Call where origin: T::Origin {
    // Initialize events for this module.
    fn deposit_event() = default;

    // Propose a listing on the registry.
    // Takes the listing name (data) as a byte vector.
    // Takes deposit as stake backing the listing.
    // Checks if the stake is less than minimum deposit needed.
    fn propose(origin, proposed_listing: ListingId, deposit: BalanceOf<T>) -> DispatchResult {
      let sender = ensure_signed(origin)?;

      // To avoid byte arrays with unlimited length.
      ensure!(data.len() <= 256, "listing data cannot be more than 256 bytes");

      let min_deposit = Self::min_deposit().ok_or("Min deposit not set")?;
      ensure!(deposit >= min_deposit, "deposit should be more than min_deposit");

      // Set application expiry for the listing.
      // Generating a future timestamp by adding the apply stage length.
      let now = <system::Module<T>>::block_number();
      let apply_stage_len = Self::apply_stage_len().ok_or("Apply stage length not set.")?;
      let app_exp = now.checked_add(&apply_stage_len).ok_or("Overflow when setting application expiry.")?;

      let hashed = <T as system::Trait>::Hashing::hash(&data);

      let listing_id = Self::listing_count();

      // Create a new listing instance and store it.
      let listing = Listing {
        id: listing_id,
        data,
        deposit,
        owner: sender.clone(),
        whitelisted: false,
        challenge_id: 0,
        application_expiry: app_exp,
      };

      ensure!(!<Listings<T>>::exists(hashed), "Listing already exists");

      // Reserve the application deposit.
	  T::Currency::reserve(&sender, deposit)
	  	.map_err(|_| "Proposer can't afford deposit")?;

      <ListingCount>::put(listing_id + 1);
      <Listings<T>>::insert(hashed, listing);
      <ListingIndexHash<T>>::insert(listing_id, hashed);

      // Let the world know.
      // Raise the event.
      Self::deposit_event(RawEvent::Proposed(sender, hashed.clone(), deposit));
      print("Listing created!");

      Ok(())
    }

    // Challenge a listing.
    // For simplicity, only three checks are being done.
    //    a. If the listing exists.
    //    c. If the challenger is not the owner of the listing.
    //    b. If enough deposit is sent for challenge.
    fn challenge(origin, listing_id: u32, deposit: BalanceOf<T>) -> DispatchResult {
      let sender = ensure_signed(origin)?;

      ensure!(<ListingIndexHash<T>>::exists(listing_id), "Listing not found.");

      let listing_hash = Self::index_hash(listing_id);
      let listing = Self::listings(listing_hash);

      ensure!(listing.challenge_id == 0, "Listing is already challenged.");
      ensure!(listing.owner != sender, "You cannot challenge your own listing.");
      ensure!(deposit >= listing.deposit, "Not enough deposit to challenge.");

      // Get current block height.
      let now = <system::Module<T>>::block_number();

      // Get commit stage length.
      let commit_stage_len = Self::commit_stage_len().ok_or("Commit stage length not set.")?;
      let voting_exp = now.checked_add(&commit_stage_len).ok_or("Overflow when setting voting expiry.")?;

      // Check apply stage length not passed.
      // Ensure listing.application_expiry < now.
      ensure!(listing.application_expiry > now, "Apply stage length has passed.");

      let challenge = Challenge {
        listing_hash,
        deposit,
        owner: sender.clone(),
        voting_ends: voting_exp,
        resolved: false,
        reward_pool: 0u32.into(),
        total_tokens: 0u32.into(),
      };

      let poll = Poll {
        listing_hash,
        votes_for: listing.deposit,
        votes_against: deposit,
        passed: false,
      };

      // Reserve the deposit for challenge.
	  T::Currency::reserve(&sender, deposit)
	    .map_err(|_| "Challenger can't afford the deposit")?;

      // Global poll nonce.
      // Helps keep the count of challenges and in maping votes.
      let poll_nonce = <PollNonce>::get();

      // Add a new challenge and the corresponding poll in the respective collections.
      <Challenges<T>>::insert(poll_nonce, challenge);
      <Polls<T>>::insert(poll_nonce, poll);

      // Update listing with challenge id.
      <Listings<T>>::mutate(listing_hash, |listing| {
        listing.challenge_id = poll_nonce;
      });

      // Update the poll nonce.
      <PollNonce>::put(poll_nonce + 1);

      // Raise the event.
      Self::deposit_event(RawEvent::Challenged(sender, listing_hash, poll_nonce, deposit));
      print("Challenge created!");

      Ok(())
    }

    // Registers a vote for a particular challenge.
    // Checks if the listing is challenged, and
    // if the commit stage length has not passed.
    // To keep it simple, we just store the choice as a bool - true: aye; false: nay.
    fn vote(origin, challenge_id: u32, value: bool, #[compact] deposit: BalanceOf<T>) -> DispatchResult {
      let sender = ensure_signed(origin)?;

      // Check if listing is challenged.
      ensure!(<Challenges<T>>::exists(challenge_id), "Challenge does not exist.");
      let challenge = Self::challenges(challenge_id);
      ensure!(challenge.resolved == false, "Challenge is already resolved.");

      // Check commit stage length not passed.
      let now = <system::Module<T>>::block_number();
      ensure!(challenge.voting_ends > now, "Commit stage length has passed.");

      // Deduct the deposit for vote.
	  T::Currency::reserve(&sender, deposit)
	    .map_err(|_| "Voter can't afford the deposit")?;

      let mut poll_instance = Self::polls(challenge_id);
      // Based on vote value, increase the count of votes (for or against).
      match value {
        true => poll_instance.votes_for += deposit,
        false => poll_instance.votes_against += deposit,
      }

      // Create a new vote instance with the input params.
      let vote_instance = Vote {
        value,
        deposit,
        claimed: false,
      };

      // Mutate polls collection to update the poll instance.
      <Polls<T>>::mutate(challenge_id, |poll| *poll = poll_instance);

      // Insert new vote into votes collection.
      <Votes<T>>::insert((challenge_id, sender.clone()), vote_instance);

      // Raise the event.
      Self::deposit_event(RawEvent::Voted(sender, challenge_id, deposit));
      print("Vote created!");
      Ok(())
    }

    // Resolves the status of a listing.
    // Changes the value of whitelisted to either true or false.
    // Checks if the listing is challenged or not.
    // Further checks if apply stage or commit stage has passed.
    // Compares if votes are in favour of whitelisting.
    // Updates the listing status.
    fn resolve(_origin, listing_id: u32) -> DispatchResult {
      ensure!(<ListingIndexHash<T>>::exists(listing_id), "Listing not found.");

      let listing_hash = Self::index_hash(listing_id);
      let listing = Self::listings(listing_hash);

      let now = <system::Module<T>>::block_number();
      let challenge;
      let poll;

      // Check if listing is challenged.
      if listing.challenge_id > 0 {
        // Challenge.
        challenge = Self::challenges(listing.challenge_id);
        poll = Self::polls(listing.challenge_id);

        // Check commit stage length has passed.
        ensure!(challenge.voting_ends < now, "Commit stage length has not passed.");
      } else {
        // No challenge.
        // Check if apply stage length has passed.
        ensure!(listing.application_expiry < now, "Apply stage length has not passed.");

        // Update listing status.
        <Listings<T>>::mutate(listing_hash, |listing|
        {
          listing.whitelisted = true;
        });

        Self::deposit_event(RawEvent::Accepted(listing_hash));
        return Ok(());
      }

      let mut whitelisted = false;

      // Mutate polls collection to update the poll instance.
      <Polls<T>>::mutate(listing.challenge_id, |poll| {
        if poll.votes_for >= poll.votes_against {
            poll.passed = true;
            whitelisted = true;
        } else {
            poll.passed = false;
        }
      });

      // Update listing status.
      <Listings<T>>::mutate(listing_hash, |listing| {
        listing.whitelisted = whitelisted;
        listing.challenge_id = 0;
      });

      // Update challenge.
      <Challenges<T>>::mutate(listing.challenge_id, |challenge| {
        challenge.resolved = true;
        if whitelisted == true {
          challenge.total_tokens = poll.votes_for;
          challenge.reward_pool = challenge.deposit + poll.votes_against;
        } else {
          challenge.total_tokens = poll.votes_against;
          challenge.reward_pool = listing.deposit + poll.votes_for;
        }
      });

      // Raise appropriate event as per whitelisting status.
      if whitelisted == true {
        Self::deposit_event(RawEvent::Accepted(listing_hash));
      } else {
        // If rejected, give challenge deposit back to the challenger.
		T::Currency::unreserve(&challenge.owner, challenge.deposit);
        Self::deposit_event(RawEvent::Rejected(listing_hash));
      }

      Self::deposit_event(RawEvent::Resolved(listing_hash, listing.challenge_id));
      Ok(())
    }

    // Claim reward for a vote.
    fn claim_reward(origin, challenge_id: u32) -> DispatchResult {
      let sender = ensure_signed(origin)?;

      // Ensure challenge exists and has been resolved.
      ensure!(<Challenges<T>>::exists(challenge_id), "Challenge not found.");
      let challenge = Self::challenges(challenge_id);
      ensure!(challenge.resolved == true, "Challenge is not resolved.");

      // Get the poll and vote instances.
      // Reward depends on poll passed status and vote value.
      let poll = Self::polls(challenge_id);
      let vote = Self::votes((challenge_id, sender.clone()));

      // Ensure vote reward is not already claimed.
      ensure!(vote.claimed == false, "Vote reward has already been claimed.");

      // If winning party, calculate reward and transfer.
      if poll.passed == vote.value {
            let reward_ratio = challenge.reward_pool.checked_div(&challenge.total_tokens).ok_or("overflow in calculating reward")?;
            let reward = reward_ratio.checked_mul(&vote.deposit).ok_or("overflow in calculating reward")?;
            let total = reward.checked_add(&vote.deposit).ok_or("overflow in calculating reward")?;
			T::Currency::unreserve(&sender, total);
            Self::deposit_event(RawEvent::Claimed(sender.clone(), challenge_id));
        }

        // Update vote reward claimed status.
        <Votes<T>>::mutate((challenge_id, sender), |vote| vote.claimed = true);

      Ok(())
    }

    // Sets the TCR parameters.
    // Currently only min deposit, apply stage length and commit stage length are supported.
    fn set_config(origin,
      min_deposit: BalanceOf<T>,
      apply_stage_len: T::BlockNumber,
      commit_stage_len: T::BlockNumber) -> DispatchResult {

      ensure_root(origin)?;

      <MinDeposit<T>>::put(min_deposit);
      <ApplyStageLen<T>>::put(apply_stage_len);
      <CommitStageLen<T>>::put(commit_stage_len);

      Ok(())
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  use primitives::{Blake2Hasher, H256};
  use runtime_io::with_externalities;
  use runtime_primitives::{
    testing::{Digest, DigestItem, Header, UintAuthorityId},
    traits::{BlakeTwo256, IdentityLookup},
    BuildStorage,
  };
  use support::{assert_noop, assert_ok, impl_outer_origin};

  impl_outer_origin! {
    pub enum Origin for Test {}
  }

  // For testing the module, we construct most of a mock runtime. This means
  // first constructing a configuration type (`Test`) which `impl`s each of the
  // configuration traits of modules we want to use.
  #[derive(Clone, Eq, PartialEq)]
  pub struct Test;
  impl system::Trait for Test {
    type Origin = Origin;
    type Index = u64;
    type BlockNumber = u64;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type Digest = Digest;
    type AccountId = u64;
    type Lookup = IdentityLookup<u64>;
    type Header = Header;
    type Event = ();
    type Log = DigestItem;
  }
  impl consensus::Trait for Test {
    type Log = DigestItem;
    type SessionKey = UintAuthorityId;
    type InherentOfflineReport = ();
  }
  impl token::Trait for Test {
    type Event = ();
    type TokenBalance = u64;
  }
  impl timestamp::Trait for Test {
    type Moment = u64;
    type OnTimestampSet = ();
  }
  impl Trait for Test {
    type Event = ();
  }
  type Tcr = Module<Test>;
  type Token = token::Module<Test>;

  // Builds the genesis config store and sets mock values.
  fn new_test_ext() -> runtime_io::TestExternalities<Blake2Hasher> {
    let mut t = system::GenesisConfig::<Test>::default()
      .build_storage()
      .unwrap()
      .0;
    t.extend(
      token::GenesisConfig::<Test> { total_supply: 1000 }
        .build_storage()
        .unwrap()
        .0,
    );
    t.extend(
      GenesisConfig::<Test> {
        owner: 1,
        min_deposit: 100,
        apply_stage_len: 10,
        commit_stage_len: 10,
        poll_nonce: 1,
      }
      .build_storage()
      .unwrap()
      .0,
    );
    t.into()
  }

  #[test]
  fn should_fail_low_deposit() {
    with_externalities(&mut new_test_ext(), || {
      assert_noop!(
        Tcr::propose(Origin::signed(1), "ListingItem1".as_bytes().into(), 99),
        "deposit should be more than min_deposit"
      );
    });
  }

  #[test]
  fn should_init() {
    with_externalities(&mut new_test_ext(), || {
      assert_ok!(Tcr::init(Origin::signed(1)));
    });
  }

  #[test]
  fn should_pass_propose() {
    with_externalities(&mut new_test_ext(), || {
      assert_ok!(Tcr::init(Origin::signed(1)));
      assert_ok!(Tcr::propose(
        Origin::signed(1),
        "ListingItem1".as_bytes().into(),
        101
      ));
    });
  }

  #[test]
  fn should_fail_challenge_same_owner() {
    with_externalities(&mut new_test_ext(), || {
      assert_ok!(Tcr::init(Origin::signed(1)));
      assert_ok!(Tcr::propose(
        Origin::signed(1),
        "ListingItem1".as_bytes().into(),
        101
      ));
      assert_noop!(
        Tcr::challenge(Origin::signed(1), 0, 101),
        "You cannot challenge your own listing."
      );
    });
  }

  #[test]
  fn should_pass_challenge() {
    with_externalities(&mut new_test_ext(), || {
      assert_ok!(Tcr::init(Origin::signed(1)));
      assert_ok!(Tcr::propose(
        Origin::signed(1),
        "ListingItem1".as_bytes().into(),
        101
      ));
      assert_ok!(Token::transfer(Origin::signed(1), 2, 200));
      assert_ok!(Tcr::challenge(Origin::signed(2), 0, 101));
    });
  }
}
