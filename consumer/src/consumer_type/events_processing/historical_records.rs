use crate::{
    consumer_type::sqs_hibrid::SqsHibrid, error::ConsumerError,
    mode::raw::models::cursor::HistoFluxCursor, traits::BasicConsumer,
};
use models::raw_logs::RawLog;
use tracing::info;

impl SqsHibrid {
    /// This function returns the page size based on the amount of logs. If the
    /// amount of logs is less than 100, it returns the amount of logs. Otherwise,
    /// it returns 100.
    fn get_page_size(amount_of_logs: i64) -> i64 {
        if amount_of_logs < 100 {
            amount_of_logs
        } else {
            100
        }
    }

    /// This function returns the ceiling division of two numbers.
    fn ceiling_div(a: i64, b: i64) -> i64 {
        if (a > 0) == (b > 0) {
            // Same signs: use regular ceiling division
            let result = (a.abs() + b.abs() - 1) / b.abs();
            if a < 0 && b < 0 {
                result // When both negative, result is positive
            } else {
                result * if a < 0 { -1 } else { 1 }
            }
        } else {
            // Different signs: use floor division
            a / b
        }
    }

    /// This function processes all existing records in the database and sends
    /// them to the SQS queue.
    pub async fn process_historical_records(&self) -> Result<(), ConsumerError> {
        // Get the last processed id from the database, if it doesnt exist,
        // it will return the default value.
        info!("Getting last processed id from the DB");
        let mut last_processed_id =
            HistoFluxCursor::find(&self.histoflux_pg_pool, &self.histoflux_cursor.environment)
                .await?
                .ok_or(ConsumerError::NotFound)?
                .last_processed_id;
        info!("Last processed id: {}", last_processed_id);
        let amount_of_logs =
            RawLog::get_total_count(&self.histoflux_pg_pool, &self.app_config.indexer_schema)
                .await?;
        // If there are no logs, we dont need to process anything
        if amount_of_logs == 0 {
            return Ok(());
        }
        let page_size = Self::get_page_size(amount_of_logs);
        let pages = Self::ceiling_div(amount_of_logs, page_size);
        info!("Processing {} pages with page size {}", pages, page_size);

        // Initialize the processed logs counter. This is used to avoid processing
        // more logs than the total amount we initially got. We have a listener
        // that will send us new logs, so we dont need to process all logs.
        let mut processed_logs_counter = 0;

        'outer_loop: for _page in 0..pages {
            let logs = RawLog::get_paginated_after_id(
                &self.histoflux_pg_pool,
                last_processed_id as i32,
                page_size,
                &self.app_config.indexer_schema,
            )
            .await?;

            info!("Processing {} logs", logs.len());
            for log in logs {
                // Don't process more logs than the total amount we initially got.
                if processed_logs_counter >= amount_of_logs {
                    break 'outer_loop;
                }
                info!("Processing log: {:?}", log);
                // Update the last processed id variable
                last_processed_id = log.id as i64;
                self.update_last_processed_id(last_processed_id).await?;
                // Send the log to the SQS queue
                let message = serde_json::to_string(&log)?;
                self.send_message(message, Some("raw".to_string())).await?;
                // Increment the processed logs counter
                processed_logs_counter += 1;
            }
        }

        Ok(())
    }

    /// This function updates the last processed id in the database.
    pub async fn update_last_processed_id(
        &self,
        last_processed_id: i64,
    ) -> Result<(), ConsumerError> {
        HistoFluxCursor::update_last_processed_id(
            &self.histoflux_pg_pool,
            &self.histoflux_cursor.environment,
            last_processed_id,
        )
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ceiling_div() {
        // Even division cases
        assert_eq!(SqsHibrid::ceiling_div(10, 2), 5);
        assert_eq!(SqsHibrid::ceiling_div(100, 10), 10);
        assert_eq!(SqsHibrid::ceiling_div(2, 100), 1);

        // Uneven division cases (should round up)
        assert_eq!(SqsHibrid::ceiling_div(11, 2), 6);
        assert_eq!(SqsHibrid::ceiling_div(99, 10), 10);

        // Edge cases
        assert_eq!(SqsHibrid::ceiling_div(1, 1), 1);
        assert_eq!(SqsHibrid::ceiling_div(0, 5), 0);

        // Large numbers
        assert_eq!(SqsHibrid::ceiling_div(1000000, 3), 333334);

        // Negative numbers (following integer division rules)
        assert_eq!(SqsHibrid::ceiling_div(-10, 3), -3);
        assert_eq!(SqsHibrid::ceiling_div(10, -3), -3);
        assert_eq!(SqsHibrid::ceiling_div(-10, -3), 4);
    }
}
