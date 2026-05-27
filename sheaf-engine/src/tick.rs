//! Tick data types — Arrow-native columnar format.

use arrow::array::{Float64Array, StringArray, TimestampNanosecondArray};
use arrow::datatypes::{DataType, Field, Schema, TimeUnit};
use arrow::record_batch::RecordBatch as ArrowRecordBatch;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct Tick {
    pub venue: String,
    pub symbol: String,
    pub price: f64,
    pub size: f64,
    pub event_ts: i64,
    pub ts: i64,
}

#[derive(Debug, Clone)]
pub struct TickBatch {
    pub batch: ArrowRecordBatch,
    pub window_close_ts: i64,
    pub raw_count: usize,
}

impl TickBatch {
    pub fn schema() -> Schema {
        Schema::new(vec![
            Field::new("venue", DataType::Utf8, false),
            Field::new("symbol", DataType::Utf8, false),
            Field::new("price", DataType::Float64, false),
            Field::new("size", DataType::Float64, false),
            Field::new(
                "event_ts",
                DataType::Timestamp(TimeUnit::Nanosecond, None),
                false,
            ),
            Field::new(
                "ts",
                DataType::Timestamp(TimeUnit::Nanosecond, None),
                false,
            ),
        ])
    }

    pub fn from_ticks(ticks: &[Tick], window_close_ts: i64) -> Self {
        let raw_count = ticks.len();

        let venues: Vec<&str> = ticks.iter().map(|t| t.venue.as_str()).collect();
        let symbols: Vec<&str> = ticks.iter().map(|t| t.symbol.as_str()).collect();
        let prices: Vec<f64> = ticks.iter().map(|t| t.price).collect();
        let sizes: Vec<f64> = ticks.iter().map(|t| t.size).collect();
        let event_tss: Vec<i64> = ticks.iter().map(|t| t.event_ts).collect();
        let tss: Vec<i64> = ticks.iter().map(|t| t.ts).collect();

        let batch = ArrowRecordBatch::try_new(
            Arc::new(Self::schema()),
            vec![
                Arc::new(StringArray::from(venues)),
                Arc::new(StringArray::from(symbols)),
                Arc::new(Float64Array::from(prices)),
                Arc::new(Float64Array::from(sizes)),
                Arc::new(TimestampNanosecondArray::from(event_tss)),
                Arc::new(TimestampNanosecondArray::from(tss)),
            ],
        )
        .expect("TickBatch schema mismatch");

        Self {
            batch,
            window_close_ts,
            raw_count,
        }
    }

    pub fn into_polars(self) -> polars::prelude::DataFrame {
        // TODO: proper Arrow→Polars conversion.
        // For scaffold, return empty DataFrame — conversion requires
        // Box<dyn Array> which needs ArrayRef → owned conversion.
        let _ = self;
        polars::prelude::DataFrame::empty()
    }

    pub fn len(&self) -> usize {
        self.batch.num_rows()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
