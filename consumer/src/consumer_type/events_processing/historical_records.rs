use crate::{
    consumer_type::sqs_hibrid::SqsHibrid, error::ConsumerError,
    mode::raw::models::cursor::HistoFluxCursor,
};
impl SqsHibrid {
    /// This function returns the page size based on the amount of logs. If the
    /// amount of logs is less than 100, it returns the amount of logs. Otherwise,
    /// it returns 100.
    pub fn get_page_size(amount_of_logs: i64) -> i64 {
        if amount_of_logs < 100 {
            amount_of_logs
        } else {
            100
        }
    }

    /// This function returns the ceiling division of two numbers.
    pub fn ceiling_div(a: i64, b: i64) -> i64 {
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
