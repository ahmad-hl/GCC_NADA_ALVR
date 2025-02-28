use std::{collections::VecDeque, time::Duration};

pub struct SlidingWindowAverage<T> {
    history_buffer: VecDeque<T>,
    max_history_size: usize,
}

impl<T> SlidingWindowAverage<T> {
    pub fn new(initial_value: T, max_history_size: usize) -> Self {
        Self {
            history_buffer: [initial_value].into_iter().collect(),
            max_history_size,
        }
    }

    pub fn submit_sample(&mut self, sample: T) {
        if self.history_buffer.len() >= self.max_history_size {
            self.history_buffer.pop_front();
        }

        self.history_buffer.push_back(sample);
    }

    pub fn retain(&mut self, count: usize) {
        self.history_buffer
            .drain(0..self.history_buffer.len().saturating_sub(count));
    }

    // Method to return an iterator over the history_buffer
    pub fn get_history_iter(&self) -> std::collections::vec_deque::Iter<'_, T> {
        self.history_buffer.iter()
    }
}

impl SlidingWindowAverage<f32> {
    pub fn get_average(&self) -> f32 {
        self.history_buffer.iter().sum::<f32>() / self.history_buffer.len() as f32
    }
}

impl SlidingWindowAverage<Duration> {
    pub fn get_average(&self) -> Duration {
        self.history_buffer.iter().sum::<Duration>() / self.history_buffer.len() as u32
    }
}


impl SlidingWindowAverage<i64> {
    pub fn get_average(&self) -> i64 {
        self.history_buffer.iter().sum::<i64>() / self.history_buffer.len() as i64
    }
}