use soroban_sdk::{
    contract, contractimpl, contracttype, Address, Env, Map, String, Symbol, Vec, U256,
};

#[contract]
pub struct MerchantNetwork;

#[contracttype]
#[derive(Clone)]
pub struct Merchant {
    pub id: String,
    pub name: String,
    pub owner: Address,
    pub business_type: String,
    pub location: Location,
    pub contact_info: String,
    pub registration_date: u64,
    pub is_verified: bool,
    pub verification_documents: Vec<String>,
    pub stellar_toml_url: String,
    pub accepted_tokens: Vec<String>,
    pub daily_limit: U256,
    pub monthly_limit: U256,
    pub current_month_volume: U256,
    pub reputation_score: u32,
    pub is_active: bool,
}

#[contracttype]
#[derive(Clone)]
pub struct MerchantRegistrationInput {
    pub name: String,
    pub business_type: String,
    pub location: Location,
    pub contact_info: String,
    pub stellar_toml_url: String,
    pub accepted_tokens: Vec<String>,
    pub daily_limit: U256,
    pub monthly_limit: U256,
    pub verification_documents: Vec<String>,
}

#[contracttype]
#[derive(Clone)]
pub struct Location {
    pub latitude_e6: i64,
    pub longitude_e6: i64,
    pub address: String,
    pub city: String,
    pub country: String,
    pub postal_code: String,
}

#[contracttype]
#[derive(Clone)]
pub struct Transaction {
    pub id: String,
    pub merchant_id: String,
    pub beneficiary_id: String,
    pub amount: U256,
    pub token: String,
    pub timestamp: u64,
    pub purpose: String,
    pub merchant_signature: String,
    pub beneficiary_signature: String,
    pub is_settled: bool,
}

#[contractimpl]
impl MerchantNetwork {
    pub fn register_merchant(
        env: Env,
        owner: Address,
        merchant_id: String,
        input: MerchantRegistrationInput,
    ) {
        owner.require_auth();

        let merchants_key = Symbol::new(&env, "merchants");
        let mut merchants: Map<String, Merchant> = env
            .storage()
            .instance()
            .get(&merchants_key)
            .unwrap_or(Map::new(&env));

        if merchants.contains_key(merchant_id.clone()) {
            panic!("merchant exists");
        }

        let merchant = Merchant {
            id: merchant_id.clone(),
            name: input.name,
            owner,
            business_type: input.business_type,
            location: input.location,
            contact_info: input.contact_info,
            registration_date: env.ledger().timestamp(),
            is_verified: false,
            verification_documents: input.verification_documents,
            stellar_toml_url: input.stellar_toml_url,
            accepted_tokens: input.accepted_tokens,
            daily_limit: input.daily_limit,
            monthly_limit: input.monthly_limit,
            current_month_volume: U256::from_u32(&env, 0),
            reputation_score: 50,
            is_active: false,
        };

        merchants.set(merchant_id.clone(), merchant);
        env.storage().instance().set(&merchants_key, &merchants);

        let queue_key = Symbol::new(&env, "verification_queue");
        let mut queue: Vec<String> = env
            .storage()
            .instance()
            .get(&queue_key)
            .unwrap_or(Vec::new(&env));
        queue.push_back(merchant_id);
        env.storage().instance().set(&queue_key, &queue);
    }

    pub fn verify_merchant(
        env: Env,
        verifier: Address,
        merchant_id: String,
        approved: bool,
        _notes: String,
    ) {
        verifier.require_auth();

        let merchants_key = Symbol::new(&env, "merchants");
        let mut merchants: Map<String, Merchant> = env
            .storage()
            .instance()
            .get(&merchants_key)
            .unwrap_or(Map::new(&env));

        if let Some(mut merchant) = merchants.get(merchant_id.clone()) {
            if approved {
                merchant.is_verified = true;
                merchant.is_active = true;
            }
            merchants.set(merchant_id.clone(), merchant);
            env.storage().instance().set(&merchants_key, &merchants);
        }

        let queue_key = Symbol::new(&env, "verification_queue");
        let queue: Vec<String> = env
            .storage()
            .instance()
            .get(&queue_key)
            .unwrap_or(Vec::new(&env));
        let mut new_queue = Vec::new(&env);
        for id in queue.iter() {
            if id != merchant_id {
                new_queue.push_back(id);
            }
        }
        env.storage().instance().set(&queue_key, &new_queue);
    }

    pub fn process_payment(
        env: Env,
        merchant: Address,
        beneficiary: Address,
        merchant_id: String,
        beneficiary_id: String,
        amount: U256,
        token: String,
        purpose: String,
    ) -> String {
        merchant.require_auth();
        beneficiary.require_auth();

        let merchants_key = Symbol::new(&env, "merchants");
        let mut merchants: Map<String, Merchant> = env
            .storage()
            .instance()
            .get(&merchants_key)
            .unwrap_or(Map::new(&env));

        let mut merchant_profile = merchants
            .get(merchant_id.clone())
            .unwrap_or_else(|| panic!("merchant not found"));

        if !merchant_profile.is_active {
            panic!("merchant inactive");
        }
        if !merchant_profile.accepted_tokens.contains(token.clone()) {
            panic!("token not accepted");
        }
        if amount > merchant_profile.daily_limit {
            panic!("daily limit");
        }
        if merchant_profile.current_month_volume.add(&amount) > merchant_profile.monthly_limit {
            panic!("monthly limit");
        }

        let tx = Transaction {
            id: String::from_str(&env, "tx"),
            merchant_id: merchant_id.clone(),
            beneficiary_id,
            amount: amount.clone(),
            token,
            timestamp: env.ledger().timestamp(),
            purpose,
            merchant_signature: String::from_str(&env, "merchant_signed"),
            beneficiary_signature: String::from_str(&env, "beneficiary_signed"),
            is_settled: false,
        };

        let tx_key = Symbol::new(&env, "merchant_transactions");
        let mut tx_by_merchant: Map<String, Vec<Transaction>> = env
            .storage()
            .instance()
            .get(&tx_key)
            .unwrap_or(Map::new(&env));

        let mut txs = tx_by_merchant
            .get(merchant_id.clone())
            .unwrap_or(Vec::new(&env));
        txs.push_back(tx);
        tx_by_merchant.set(merchant_id.clone(), txs);
        env.storage().instance().set(&tx_key, &tx_by_merchant);

        merchant_profile.current_month_volume = merchant_profile.current_month_volume.add(&amount);
        merchants.set(merchant_id, merchant_profile);
        env.storage().instance().set(&merchants_key, &merchants);

        String::from_str(&env, "tx")
    }

    pub fn get_merchant(env: Env, merchant_id: String) -> Option<Merchant> {
        let merchants_key = Symbol::new(&env, "merchants");
        let merchants: Map<String, Merchant> = env
            .storage()
            .instance()
            .get(&merchants_key)
            .unwrap_or(Map::new(&env));
        merchants.get(merchant_id)
    }

    pub fn find_merchants_by_location(
        env: Env,
        latitude_e6: i64,
        longitude_e6: i64,
        radius_e6: i64,
    ) -> Vec<Merchant> {
        let merchants_key = Symbol::new(&env, "merchants");
        let merchants: Map<String, Merchant> = env
            .storage()
            .instance()
            .get(&merchants_key)
            .unwrap_or(Map::new(&env));

        let mut nearby = Vec::new(&env);
        for (_, merchant) in merchants.iter() {
            if merchant.is_active {
                let distance = Self::calculate_distance(
                    latitude_e6,
                    longitude_e6,
                    merchant.location.latitude_e6,
                    merchant.location.longitude_e6,
                );
                if distance <= radius_e6 {
                    nearby.push_back(merchant);
                }
            }
        }
        nearby
    }

    fn calculate_distance(lat1_e6: i64, lon1_e6: i64, lat2_e6: i64, lon2_e6: i64) -> i64 {
        let dlat = if lat2_e6 >= lat1_e6 {
            lat2_e6 - lat1_e6
        } else {
            lat1_e6 - lat2_e6
        };
        let dlon = if lon2_e6 >= lon1_e6 {
            lon2_e6 - lon1_e6
        } else {
            lon1_e6 - lon2_e6
        };
        dlat + dlon
    }

    pub fn get_merchant_transactions(env: Env, merchant_id: String) -> Vec<Transaction> {
        let tx_key = Symbol::new(&env, "merchant_transactions");
        let tx_by_merchant: Map<String, Vec<Transaction>> = env
            .storage()
            .instance()
            .get(&tx_key)
            .unwrap_or(Map::new(&env));
        tx_by_merchant.get(merchant_id).unwrap_or(Vec::new(&env))
    }

    pub fn update_reputation(env: Env, admin: Address, merchant_id: String, feedback_score: i32) {
        admin.require_auth();

        let merchants_key = Symbol::new(&env, "merchants");
        let mut merchants: Map<String, Merchant> = env
            .storage()
            .instance()
            .get(&merchants_key)
            .unwrap_or(Map::new(&env));

        if let Some(mut merchant) = merchants.get(merchant_id.clone()) {
            let new_score = (merchant.reputation_score as i32 + feedback_score)
                .max(0)
                .min(100);
            merchant.reputation_score = new_score as u32;
            merchants.set(merchant_id, merchant);
            env.storage().instance().set(&merchants_key, &merchants);
        }
    }

    pub fn reset_monthly_volumes(env: Env) {
        let merchants_key = Symbol::new(&env, "merchants");
        let mut merchants: Map<String, Merchant> = env
            .storage()
            .instance()
            .get(&merchants_key)
            .unwrap_or(Map::new(&env));

        for (merchant_id, mut merchant) in merchants.iter() {
            merchant.current_month_volume = U256::from_u32(&env, 0);
            merchants.set(merchant_id, merchant);
        }
        env.storage().instance().set(&merchants_key, &merchants);
    }

    pub fn get_verification_queue(env: Env) -> Vec<String> {
        let queue_key = Symbol::new(&env, "verification_queue");
        env.storage()
            .instance()
            .get(&queue_key)
            .unwrap_or(Vec::new(&env))
    }
}
