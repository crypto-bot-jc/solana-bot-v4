// use crate::financial_services::price_in_sol_data::{PRICE_IN_SOL_DATA, PriceInSolItem};

// pub struct PriceInSolResult {
//     initial_price_in_sol: f64,
//     initial_token_amount: f64,
//     initial_token_reserve: f64,
//     slipage: f64,
//     search_index: usize,
//     price_in_sol: f64,
//     token_amount: f64,
//     token_reserve: f64,
//     sol_amount: f64,
//     uncorrected_sol_amount: f64,
// }

// struct PriceInSolSearchResult {
//     index: usize,
//     ratio: f64,
// }

// pub struct PriceInSolFinder;

// impl PriceInSolFinder {
//     pub async fn find_price_in_sol(
//         &self,
//         initial_price_in_sol: f64,
//         initial_token_amount: f64,
//         initial_token_reserve: f64,
//         token_amount: f64,
//         slipage: f64,
//     ) -> PriceInSolResult {
//         let price_in_sol_data: [PriceInSolItem; 793] = PRICE_IN_SOL_DATA;
//         let step_token_amount = 1_000_000.0;
//         let correction_factor = slipage; // -0.003
//         let initial_search = self.binary_find_row(initial_price_in_sol, &price_in_sol_data);

//         // first step
//         let mut current_step_index = initial_search.index;
//         let mut current_step_ratio = initial_search.ratio;
//         let mut current_step_price_in_sol = self.mid_point(
//             price_in_sol_data[current_step_index].price_in_sol,
//             price_in_sol_data[current_step_index + 1].price_in_sol,
//             current_step_ratio,
//         );
//         let mut current_step_token_amount =
//             self.mid_point(0.0, step_token_amount, current_step_ratio);
//         let mut sol_amount = current_step_price_in_sol * current_step_token_amount;

//         // loop thru next steps
//         while (current_step_token_amount + step_token_amount) < token_amount {
//             current_step_index += 1;
//             current_step_ratio = 1.0;
//             current_step_price_in_sol = price_in_sol_data[current_step_index].price_in_sol;
//             current_step_token_amount += step_token_amount;
//             sol_amount += current_step_price_in_sol * step_token_amount;
//         }

//         // last step
//         if current_step_token_amount < token_amount {
//             current_step_index += 1;
//             current_step_ratio = (token_amount - current_step_token_amount) / step_token_amount;
//             current_step_price_in_sol = self.mid_point(
//                 price_in_sol_data[current_step_index],
//                 price_in_sol_data[current_step_index + 1],
//                 current_step_ratio,
//             );
//             current_step_token_amount = token_amount - current_step_token_amount;
//             sol_amount += current_step_price_in_sol * current_step_token_amount;
//         }

//         let corrected_sol_amount = sol_amount * (1.0 + correction_factor);

//         PriceInSolResult {
//             initial_price_in_sol,
//             initial_token_amount,
//             initial_token_reserve,
//             slipage,
//             search_index: initial_search.index,
//             price_in_sol: corrected_sol_amount / token_amount,
//             token_amount,
//             token_reserve: initial_token_reserve - token_amount,
//             sol_amount: corrected_sol_amount,
//             uncorrected_sol_amount: sol_amount,
//         }
//     }

//     fn mid_point(
//         &self,
//         lower_price_in_sol: f64,
//         higher_price_in_sol: f64,
//         displacement_ratio: f64,
//     ) -> f64 {
//         lower_price_in_sol * (1.0 - displacement_ratio) + higher_price_in_sol * displacement_ratio
//     }

//     fn binary_find_row(
//         &self,
//         price_in_sol: f64,
//         price_in_sol_data: &[PriceInSolItem],
//     ) -> PriceInSolSearchResult {
//         let mut lower_bound_index = 0;
//         let mut higher_bound_index = price_in_sol_data.len() - 1;
//         let mut mid_point_index = 0;

//         while lower_bound_index <= higher_bound_index {
//             mid_point_index = (lower_bound_index + higher_bound_index) / 2;

//             if price_in_sol_data[mid_point_index].price_in_sol == price_in_sol {
//                 break;
//             } else if price_in_sol_data[mid_point_index].price_in_sol < price_in_sol {
//                 lower_bound_index = mid_point_index + 1;
//             } else {
//                 higher_bound_index = mid_point_index - 1;
//             }
//         }

//         if mid_point_index >= price_in_sol_data.len() - 1 {
//             mid_point_index = price_in_sol_data.len() - 2;
//         }

//         let higher_price_in_sol =
//             price_in_sol_data[mid_point_index + 1] - price_in_sol_data[mid_point_index];
//         let adj_price_in_sol = price_in_sol - price_in_sol_data[mid_point_index];
//         let ratio = adj_price_in_sol / higher_price_in_sol;

//         PriceInSolSearchResult {
//             index: mid_point_index,
//             ratio,
//         }
//     }
// }

