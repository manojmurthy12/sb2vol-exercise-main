use anchor_lang::prelude::*;
use switchboard_solana::prelude::*;
declare_id!("4TtQakHeBgTRjG85RHmWs25LK37DdmfP4PnS45Hmcgd5");

#[program]
pub mod sbvol {

    use core::f64;
    use std::ptr::null;

    use super::*;

    pub fn initialize(
        _ctx: Context<Initialize>
    ) -> Result<()> {
        let my_account = &mut _ctx.accounts.stored_data;
        my_account.current_price = 0.0; 
        my_account.volatility = 0.0; 
        Ok(())
    }

    // TODO: Read price from oracle
    pub fn read_price(
        _ctx: Context<ReadPrice>,
        _params: ReadPriceParams
    ) -> anchor_lang::Result<()>     {
        
        let feed = &_ctx.accounts.aggregator.load()?;

        // get result
        let val: f64 = feed.get_result()?.try_into()?;

        // check whether the feed has been updated in the last 300 seconds
        feed.check_staleness(
            solana_program::clock::Clock::get().unwrap().unix_timestamp,
            300,
        )
        .map_err(|_| error!(ErrorCode::StaleFeed))?;

        // check feed does not exceed max_confidence_interval
        if let Some(max_confidence_interval) = _params.max_confidence_interval {
            feed.check_confidence_interval(SwitchboardDecimal::from_f64(max_confidence_interval))
                .map_err(|_| error!(ErrorCode::ConfidenceIntervalExceeded))?;
        }

        _ctx.accounts.stored_data.current_price = val;
        Ok(())
    }

    // TODO: Calculate vol from oracle 
    pub fn calc_vol(_ctx: Context<CalcVol>, _params: CalculateVolParams) -> Result<()> {
        
        let history_buffer = AggregatorHistoryBuffer::new(&_ctx.accounts.history_buffer)?;
        // Determine the time range for fetching historical data
        let clock = solana_program::clock::Clock::get().unwrap();
        let end_timestamp = _params.endtimestamp.unwrap_or(clock.unix_timestamp);
        let start_timestamp = _params.starttimestamp.unwrap_or(end_timestamp - (3600 * 24 * 10));
        
        let mut prices = Vec::new();
        
        // Set the interval from params or default to 1 day (3600 * 24)
        let interval = _params.interval.unwrap_or(3600 * 24);
        let mut previous_price: Option<f64> = None; // Use Option to avoid unnecessary initialization
        
        let mut current_timestamp = start_timestamp;
        
        while current_timestamp <= end_timestamp {
            // Fetch the value at the current timestamp
            if let Some(history_value) = history_buffer.lower_bound(current_timestamp) {
        
                let curr_val: f64 = history_value.value.try_into()?;

                // Calculate the price difference only if we have a previous price
                if let Some(prev_price) = previous_price {
                    let difference = curr_val - prev_price;
                    // Avoid division if curr_val is 0 to prevent panic
                    if curr_val != 0.0 {
                        prices.push(difference / curr_val);
                    } 
                }
                previous_price = Some(curr_val); // Update the previous price
            } 
            current_timestamp += interval;
        }
        
        // Check if we have enough data to calculate volatility
        if prices.len() < 2 {
            return Err(ErrorCode::NotEnoughData.into());
        }

        // Calculate volatility
        // let volatility = calculate_standard_deviation(&prices); //faster
        let volatility = std_deviation(&prices); //more accurate

        if interval == 3600*24 {
            let number_of_periods_in_a_year:f64 = 252.0;
            let annualized_volatility = number_of_periods_in_a_year.sqrt()*volatility.expect("No data in the range");
            msg!("Annualized volatility: {}", annualized_volatility);
        }

        _ctx.accounts.stored_data.volatility = volatility.expect("No data in the range");
        Ok(())
    }

}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        init,
        payer = user,
        space = 8 + 8 + 8
    )]
    pub stored_data: Account<'info, SwitchBoardStoredData>,
    pub system_program: Program<'info, System>
}

#[account]
pub struct SwitchBoardStoredData { // data account of the user which stores required data
    pub current_price: f64, //variable to store current price of the SPOT asset
    pub volatility:f64 // variable to store volatility of the SPOT asset
}


#[derive(Accounts)]
#[instruction(params: ReadPriceParams)]
pub struct ReadPrice<'info> {
    #[account()]
    pub aggregator: AccountLoader<'info, AggregatorAccountData>,
    #[account(mut)]
    pub stored_data: Account<'info, SwitchBoardStoredData>
}

#[derive(Accounts)]
#[instruction(params: CalculateVolParams)]
pub struct CalcVol<'info> {

    #[account(
        has_one = history_buffer @ ErrorCode::InvalidHistoryBuffer
    )]
    pub aggregator: AccountLoader<'info, AggregatorAccountData>,
    /// CHECK: verified in the aggregator has_one check
    pub history_buffer: AccountInfo<'info>,
    #[account(mut)]
    pub stored_data: Account<'info, SwitchBoardStoredData>   
}

#[derive(Clone, AnchorSerialize, AnchorDeserialize)]
pub struct ReadPriceParams {
    pub max_confidence_interval: Option<f64>,
}


#[derive(Clone, AnchorSerialize, AnchorDeserialize)]
pub struct CalculateVolParams {
    pub interval: Option<i64>, // in seconds
    pub starttimestamp: Option<i64>,
    pub endtimestamp: Option<i64>
}

#[error_code]
#[derive(Eq, PartialEq)]
pub enum ErrorCode {
    #[msg("Not a valid Switchboard account")]
    InvalidSwitchboardAccount,
    #[msg("Switchboard feed has not been updated in 5 minutes")]
    StaleFeed,
    #[msg("Switchboard feed exceeded provided confidence interval")]
    ConfidenceIntervalExceeded,
    #[msg("History buffer mismatch")]
    InvalidHistoryBuffer,
    #[msg("Mathematical operation error")]
    Math,
    #[msg("Not enough data in the range")]
    NotEnoughData
}

// A faster version of standard deviation
fn calculate_standard_deviation(prices: &[f64]) -> f64 {
    let n = prices.len() as f64;
    let mut mean = 0.0;
    let mut m2 = 0.0;

    for (i, price) in prices.iter().enumerate() {
        let delta = price - mean;
        mean += delta / (i as f64 + 1.0);
        m2 += delta * (price - mean);
    }

    // Variance is m2 / (n - 1), and standard deviation is the square root of variance
    (m2 / (n - 1.0)).sqrt()
}

fn mean(data: &[f64]) -> Option<f64> {
    let sum = data.iter().sum::<f64>() as f64;
    let count = data.len();

    match count {
        positive if positive > 0 => Some(sum / count as f64),
        _ => None,
    }
}
// accurate version of standard deviation
fn std_deviation(data: &[f64]) -> Option<f64> {
    match (mean(data), data.len()) {
        (Some(data_mean), count) if count > 0 => {
            let variance = data.iter().map(|value| {
                let diff = data_mean - (*value as f64);

                diff * diff
            }).sum::<f64>() / count as f64;

            Some(variance.sqrt())
        },
        _ => None
    }
}