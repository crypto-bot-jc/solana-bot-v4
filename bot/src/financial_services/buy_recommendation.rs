// const BUYRECOMMENDATION_UNDEFINED: u8 = 0;
// const BUYRECOMMENDATION_BUY: u8 = 1;
// const BUYRECOMMENDATION_DONOTTOUCH: u8 = 2;

// lazy_static::lazy_static! {
//     static ref BUY_MINIMUN_TRANSACTION_ON_MINT_TO_RECOMMEND: usize = env::var("BUY_MINIMUN_TRANSACTION_ON_MINT_TO_RECOMMEND").unwrap_or_else(|_| "10".to_string()).parse().unwrap();
//     static ref BUY_MINIMUN_TRANSACTION_ON_MINT_IN_LAST_MINUTE_TO_RECOMMEND: usize = env::var("BUY_MINIMUN_TRANSACTION_ON_MINT_IN_LAST_MINUTE_TO_RECOMMEND").unwrap_or_else(|_| "2".to_string()).parse().unwrap();
//     static ref BUY_MINIMUN_TREND_UP_ON_MINT_TO_RECOMMEND: f64 = env::var("BUY_MINIMUN_TREND_UP_ON_MINT_TO_RECOMMEND").unwrap_or_else(|_| "0.1".to_string()).parse().unwrap();
//     static ref BUY_MARKETCAP_MIN: f64 = env::var("BUY_MARKETCAP_MIN").unwrap_or_else(|_| "100.0".to_string()).parse().unwrap();
//     static ref BUY_MARKETCAP_MAX: f64 = env::var("BUY_MARKETCAP_MAX").unwrap_or_else(|_| "10000.0".to_string()).parse().unwrap();
//     static ref SELL_MAXIMUM_PROFIT_PERCENTAGE: f64 = env::var("SELL_MAXIMUM_PROFIT_PERCENTAGE").unwrap_or_else(|_| "0.0".to_string()).parse().unwrap();
// }

// struct BuyRecommendationRequest {
//     transactions: Transactions,
//     position_tracker: PositionTracker,
// }

// impl BuyRecommendationRequest {
//     fn is_valid(&self) -> bool {
//         // Implement the validation logic here
//         true
//     }
// }

// struct BuyRecommendationResponse {
//     request: BuyRecommendationRequest,
//     recommendation: u8,
//     reasons: Vec<String>,
//     variation: f64,
//     bcei_points: f64,
//     time_stamp: u64,
// }

// impl BuyRecommendationResponse {
//     fn new(request: BuyRecommendationRequest) -> Self {
//         Self {
//             request,
//             recommendation: BUYRECOMMENDATION_DONOTTOUCH,
//             reasons: Vec::new(),
//             variation: 0.0,
//             bcei_points: 0.0,
//             time_stamp: request.transactions.last_transaction().message_timestamp,
//         }
//     }

//     async fn print_recommendation(&self) {
//         let last_transaction = self.request.transactions.last_transaction().await;
//         println!("--------------------BUY RECOMMENDATION----------------------");
//         println!("mint: {}", self.request.transactions.mint);
//         println!("priceInSol: {}", last_transaction.price_in_sol);
//         println!("messageTimestamp epoc: {}", last_transaction.message_timestamp);
//         println!("messageTimestamp: {}", self.format_epoch_time(last_transaction.message_timestamp).await);
//         match self.recommendation {
//             BUYRECOMMENDATION_BUY => println!("recommendation: BUYRECOMMENDATION_BUY"),
//             BUYRECOMMENDATION_DONOTTOUCH => println!("recommendation: BUYRECOMMENDATION_DONOTTOUCH"),
//             _ => println!("recommendation: BUYRECOMMENDATION_UNDEFINED"),
//         }
//         println!("reasons:");
//         for reason in &self.reasons {
//             println!("\t{}", reason);
//         }
//         println!("--------------------BUY RECOMMENDATION----------------------");
//     }

//     async fn format_epoch_time(&self, epoch_time: u64) -> String {
//         let datetime = NaiveDateTime::from_timestamp(epoch_time as i64, 0);
//         datetime.format("%H:%M:%S").to_string()
//     }
// }

// struct BuyRecommendation;

// /// The `BuyRecommendation` struct provides methods to generate buy recommendations
// /// based on various criteria and market conditions.
// ///
// /// # Methods
// ///
// /// - `get_recommendation`: Generates a buy recommendation based on the provided request.
// /// - `get_recommendation_just_created`: Generates a buy recommendation for newly created mints.
// /// - `get_recommendation_pump_fun`: Generates a buy recommendation based on pump and dump criteria.
// /// - `position_end_of_descent`: Checks if the position is at the end of a descent.
// /// - `mint_has_minimum_transaction`: Checks if the mint has a minimum number of transactions.
// /// - `mint_is_trending_up_1_min`: Checks if the mint is trending up in the last 1 minute.
// /// - `mint_is_trending_up_5_min`: Checks if the mint is trending up in the last 5 minutes.
// /// - `liquidity_enough`: Checks if the liquidity is sufficient.
// /// - `mint_m_has_negative_buy_history`: Checks if the mint has a negative buy history.
// /// - `mint_market_cap_big_enough`: Checks if the mint's market cap is big enough.
// /// - `mint_just_launched_market_cap_big_enough`: Checks if the market cap of a just launched mint is within a specified range.
// /// - `mint_is_just_launched`: Checks if the mint is just launched.
// /// - `mint_just_launched_is_trending_up`: Checks if a just launched mint is trending up.
// /// - `mint_just_launched_holders_ok`: Checks if the holders of a just launched mint are within acceptable limits.
// /// - `mint_just_launched_not_too_old`: Checks if a just launched mint is not too old.
// /// - `mint_just_launched_no_major_sell`: Placeholder method for checking major sells in just launched mints.
// /// - `mint_just_launched_not_a_bot`: Placeholder method for checking bot activity in just launched mints.
// /// - `mint_just_launched_has_buy_history`: Checks if a just launched mint has a buy history.
// impl BuyRecommendation {
//     async fn get_recommendation(&self, request: BuyRecommendationRequest) -> BuyRecommendationResponse {
//         if !request.is_valid() {
//             panic!("Invalid request type");
//         }

//         let mut response = self.get_recommendation_pump_fun(request).await;
//         if response.recommendation == BUYRECOMMENDATION_DONOTTOUCH {
//             response = self.get_recommendation_just_created(request).await;
//         }
//         response
//     }

//     async fn get_recommendation_just_created(&self, request: BuyRecommendationRequest) -> BuyRecommendationResponse {
//         let mut response = BuyRecommendationResponse::new(request);
//         let mut exit_recommendation = false;

//         if !exit_recommendation {
//             exit_recommendation = !self.mint_just_launched_is_trending_up(request, &mut response).await;
//         }

//         if !exit_recommendation {
//             exit_recommendation = !self.mint_just_launched_not_too_old(request, &mut response).await;
//         }

//         if !exit_recommendation {
//             exit_recommendation = !self.mint_just_launched_has_buy_history(request, &mut response).await;
//         }

//         if !exit_recommendation {
//             exit_recommendation = !self.mint_is_just_launched(request, &mut response).await;
//         }

//         if !exit_recommendation {
//             exit_recommendation = !self.mint_just_launched_market_cap_big_enough(request, &mut response).await;
//         }

//         if !exit_recommendation {
//             exit_recommendation = !self.mint_just_launched_holders_ok(request, &mut response).await;
//         }

//         if !exit_recommendation {
//             response.recommendation = BUYRECOMMENDATION_BUY;
//         } else {
//             response.recommendation = BUYRECOMMENDATION_DONOTTOUCH;
//         }

//         response
//     }

//     async fn get_recommendation_pump_fun(&self, request: BuyRecommendationRequest) -> BuyRecommendationResponse {
//         let mut response = BuyRecommendationResponse::new(request);
//         let mut exit_recommendation = false;

//         if !exit_recommendation {
//             exit_recommendation = !self.mint_just_launched_has_buy_history(request, &mut response).await;
//         }

//         if !exit_recommendation {
//             exit_recommendation = !self.mint_market_cap_big_enough(request, &mut response).await;
//         }

//         if !exit_recommendation {
//             exit_recommendation = !self.mint_has_minimum_transaction(request, &mut response).await;
//         }

//         if !exit_recommendation {
//             exit_recommendation = !self.liquidity_enough(request, &mut response).await;
//         }

//         if !exit_recommendation {
//             exit_recommendation = !self.mint_is_trending_up_1_min(request, &mut response).await;
//         }

//         if !exit_recommendation {
//             exit_recommendation = !self.mint_is_trending_up_5_min(request, &mut response).await;
//         }

//         if !exit_recommendation {
//             exit_recommendation = !self.position_end_of_descent(request, &mut response).await;
//         }

//         if !exit_recommendation {
//             response.recommendation = BUYRECOMMENDATION_BUY;
//         } else {
//             response.recommendation = BUYRECOMMENDATION_DONOTTOUCH;
//         }

//         response
//     }

//     async fn position_end_of_descent(&self, request: BuyRecommendationRequest, response: &mut BuyRecommendationResponse) -> bool {
//         if request.transactions.stats1_min.get_end_of_descent(*BUY_MARKETCAP_MIN, *BUY_MARKETCAP_MAX).await {
//             response.reasons.push(format!("(End Of Descent: {})", request.transactions.stats1_min.end_of_descent.trace));
//             true
//         } else {
//             response.reasons.push(format!("(not End Of Descent: {})", request.transactions.stats1_min.end_of_descent.trace));
//             false
//         }
//     }

//     async fn mint_has_minimum_transaction(&self, request: BuyRecommendationRequest, response: &mut BuyRecommendationResponse) -> bool {
//         let nb_transaction = request.transactions.stats1_min.nb_tx;
//         if nb_transaction > *BUY_MINIMUN_TRANSACTION_ON_MINT_TO_RECOMMEND {
//             response.reasons.push(format!("(enough transaction({}))", nb_transaction));
//             true
//         } else {
//             response.reasons.push(format!("(not enough transaction({}))", nb_transaction));
//             false
//         }
//     }

//     async fn mint_is_trending_up_1_min(&self, request: BuyRecommendationRequest, response: &mut BuyRecommendationResponse) -> bool {
//         let min_price_in_sol = request.transactions.stats1_min.first_transaction.price_in_sol;
//         let max_price_in_sol = request.transactions.stats1_min.last_transaction.price_in_sol;
//         let variation_price_in_sol = request.transactions.stats1_min.variation_price_in_sol;
//         let profit_percentage = request.transactions.stats1_min.variation_percent_price_in_sol;

//         if profit_percentage >= *BUY_MINIMUN_TREND_UP_ON_MINT_TO_RECOMMEND {
//             response.reasons.push(format!("(TrendingUp 1 minute maxPriceInSol: {}, minPriceInSol: {}, variationPriceInSol: {}, profitPercentage: {})", max_price_in_sol, min_price_in_sol, variation_price_in_sol, profit_percentage));
//             true
//         } else {
//             response.reasons.push(format!("(Not TrendingUp 1 minute maxPriceInSol: {}, minPriceInSol: {}, variationPriceInSol: {}, profitPercentage: {})", max_price_in_sol, min_price_in_sol, variation_price_in_sol, profit_percentage));
//             false
//         }
//     }

//     async fn mint_is_trending_up_5_min(&self, request: BuyRecommendationRequest, response: &mut BuyRecommendationResponse) -> bool {
//         let min_price_in_sol = request.transactions.stats5_min.first_transaction.price_in_sol;
//         let max_price_in_sol = request.transactions.stats5_min.last_transaction.price_in_sol;
//         let variation_price_in_sol = request.transactions.stats5_min.variation_price_in_sol;
//         let profit_percentage = request.transactions.stats5_min.variation_percent_price_in_sol;

//         if profit_percentage >= *BUY_MINIMUN_TREND_UP_ON_MINT_TO_RECOMMEND {
//             response.reasons.push(format!("(TrendingUp 5 minutes maxPriceInSol: {}, minPriceInSol: {}, variationPriceInSol: {}, profitPercentage: {})", max_price_in_sol, min_price_in_sol, variation_price_in_sol, profit_percentage));
//             true
//         } else {
//             response.reasons.push(format!("(Not TrendingUp 5 minutes maxPriceInSol: {}, minPriceInSol: {}, variationPriceInSol: {}, profitPercentage: {})", max_price_in_sol, min_price_in_sol, variation_price_in_sol, profit_percentage));
//             false
//         }
//     }

//     async fn liquidity_enough(&self, request: BuyRecommendationRequest, response: &mut BuyRecommendationResponse) -> bool {
//         let last_transaction = request.transactions.stats1_min.last_transaction;
//         let real_token_reserve = last_transaction.real_token_reserve;

//         if real_token_reserve > 100 {
//             response.reasons.push(format!("(Liquidity big enough: {})", real_token_reserve));
//             true
//         } else {
//             response.reasons.push(format!("(Liquidity too small: {})", real_token_reserve));
//             false
//         }
//     }

//     async fn mint_m_has_negative_buy_history(&self, request: BuyRecommendationRequest, response: &mut BuyRecommendationResponse) -> bool {
//         if !request.position_tracker.has_position(request.transactions.mint) {
//             response.reasons.push("(No PL history for mint)".to_string());
//             true
//         } else {
//             let price_in_sol = request.position_tracker.get_price(request.transactions.mint);
//             let pl = request.position_tracker.get_pl(request.transactions.mint, price_in_sol).pl;

//             if pl < -1 {
//                 response.reasons.push(format!("(Good PL history for mint: {})", pl));
//                 true
//             } else {
//                 response.reasons.push(format!("(Bad PL history for mint: {})", pl));
//                 false
//             }
//         }
//     }

//     async fn mint_market_cap_big_enough(&self, request: BuyRecommendationRequest, response: &mut BuyRecommendationResponse) -> bool {
//         let last_transaction = request.transactions.stats1_min.last_transaction;
//         let market_cap = last_transaction.market_cap;

//         if market_cap > 10 {
//             response.reasons.push(format!("(Marketcap big enough: {})", market_cap));
//             true
//         } else {
//             response.reasons.push(format!("(Marketcap too small: {})", market_cap));
//             false
//         }
//     }

//     async fn mint_just_launched_market_cap_big_enough(&self, request: BuyRecommendationRequest, response: &mut BuyRecommendationResponse) -> bool {
//         let last_transaction = request.transactions.last_transaction;
//         let market_cap_usd = last_transaction.market_cap_usd;
//         let min_marketcap_usd = 10000;
//         let max_marketcap_usd = 20000;

//         if market_cap_usd < min_marketcap_usd {
//             response.reasons.push(format!("(Just Launched Marketcap too small (10K) : {})", market_cap_usd));
//             false
//         } else if market_cap_usd > max_marketcap_usd {
//             response.reasons.push(format!("(Just Launched Marketcap too big (20K) : {})", market_cap_usd));
//             false
//         } else {
//             response.reasons.push(format!("(Just Launched Marketcap big enough (between 10 and 20K): {})", market_cap_usd));
//             true
//         }
//     }

//     async fn mint_is_just_launched(&self, request: BuyRecommendationRequest, response: &mut BuyRecommendationResponse) -> bool {
//         if request.transactions.transaction_list.is_empty() {
//             response.reasons.push(format!("(Not enough transactions: {})", request.transactions.transaction_list.len()));
//             false
//         } else {
//             let first_transaction = request.transactions.first_transaction;
//             let last_transaction = request.transactions.last_transaction;

//             if first_transaction.is_token_creation != "PumpFun" {
//                 response.reasons.push("(Not a Just Launched Mint)".to_string());
//                 false
//             } else {
//                 let elapsed_time = last_transaction.message_timestamp - first_transaction.message_timestamp;

//                 if elapsed_time < 1000 * 30 {
//                     response.reasons.push(format!("(Just Launched Mint : {})", elapsed_time));
//                     true
//                 } else {
//                     response.reasons.push(format!("(Just Launched Mint too old: {})", elapsed_time));
//                     false
//                 }
//             }
//         }
//     }

//     async fn mint_just_launched_is_trending_up(&self, request: BuyRecommendationRequest, response: &mut BuyRecommendationResponse) -> bool {
//         let min_price_in_sol = request.transactions.stats5_min.first_transaction.price_in_sol;
//         let max_price_in_sol = request.transactions.stats5_min.last_transaction.price_in_sol;
//         let variation_price_in_sol = request.transactions.stats5_min.variation_price_in_sol;
//         let profit_percentage = request.transactions.stats5_min.variation_percent_price_in_sol;

//         let last_transaction = request.transactions.last_transaction;
//         if last_transaction.is_buy {
//             response.reasons.push(format!("(Just Launched TrendingUp maxPriceInSol: {}, minPriceInSol: {}, variationPriceInSol: {}, profitPercentage: {})", max_price_in_sol, min_price_in_sol, variation_price_in_sol, profit_percentage));
//             true
//         } else {
//             response.reasons.push(format!("(Just Launched Not TrendingUp maxPriceInSol: {}, minPriceInSol: {}, variationPriceInSol: {}, profitPercentage: {})", max_price_in_sol, min_price_in_sol, variation_price_in_sol, profit_percentage));
//             false
//         }
//     }

//     async fn mint_just_launched_holders_ok(&self, request: BuyRecommendationRequest, response: &mut BuyRecommendationResponse) -> bool {
//         let holders_summary = request.transactions.get_holders_summary;

//         if holders_summary.nb_holders < 25 {
//             response.reasons.push(format!("(Just Launched Not enough holders: {})", holders_summary.nb_holders));
//             false
//         } else {
//             let top10_ownership = holders_summary.token_amount_buy_sell_total_top10 as f64 / 1000000000.0;
//             if top10_ownership > 0.4 {
//                 response.reasons.push(format!("(Just Launched too much ownership by top10 Holders: {}, {}, {})", holders_summary.token_amount_buy_sell_total_top10, holders_summary.token_amount_buy_sell_total, top10_ownership));
//                 false
//             } else {
//                 response.reasons.push(format!("(Just Launched Holders OK: {}, {})", holders_summary.nb_holders, top10_ownership));
//                 true
//             }
//         }
//     }

//     async fn mint_just_launched_not_too_old(&self, request: BuyRecommendationRequest, response: &mut BuyRecommendationResponse) -> bool {
//         let first_transaction = request.transactions.first_transaction;
//         let last_transaction = request.transactions.last_transaction;
//         let elapsed_time_in_sec = (last_transaction.message_timestamp - first_transaction.message_timestamp) / 1000;

//         if elapsed_time_in_sec > (2 * 60) {
//             response.reasons.push(format!("(Just Launched Mint is too old {})", elapsed_time_in_sec));
//             false
//         } else {
//             response.reasons.push(format!("(Just Launched Mint is not too old {}):", elapsed_time_in_sec));
//             true
//         }
//     }

//     async fn mint_just_launched_no_major_sell(&self, request: BuyRecommendationRequest, response: &mut BuyRecommendationResponse) -> bool {
//         response.reasons.push("(Just Launched MintJustLaunchedNoMajorSell:)".to_string());
//         true
//     }

//     async fn mint_just_launched_not_a_bot(&self, request: BuyRecommendationRequest, response: &mut BuyRecommendationResponse) -> bool {
//         response.reasons.push("(Just Launched MintJustLaunchedNotABot:)".to_string());
//         true
//     }

//     async fn mint_just_launched_has_buy_history(&self, request: BuyRecommendationRequest, response: &mut BuyRecommendationResponse) -> bool {
//         let position = request.position_tracker.get_position(request.transactions.mint);
//         if position.is_valid {
//             response.reasons.push("(Just Launched Already invested in mint)".to_string());
//             false
//         } else {
//             response.reasons.push("(Just Launched No PL history for mint)".to_string());
//             true
//         }
//     }
// }

