use std::{sync::Arc, time::Duration};

use rodio::{Sample, Source};

/// When the inner source is empty calls a fn.
#[derive(Clone)]
pub struct DoneCallback<I> {
    input: I,
    signal: Arc<dyn Fn(()) + Send + Sync>,
    signal_sent: bool,
}

impl<I: std::fmt::Debug> std::fmt::Debug for DoneCallback<I> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DoneCallback")
            .field("input", &self.input)
            // .field("signal", &self.signal)
            .field("signal_sent", &self.signal_sent)
            .finish()
    }
}

impl<I> DoneCallback<I> {
    #[inline]
    pub fn new(input: I, signal: Arc<dyn Fn(()) + Send + Sync>) -> DoneCallback<I> {
        DoneCallback {
            input,
            signal,
            signal_sent: false,
        }
    }

    /// Returns a reference to the inner source.
    #[inline]
    #[allow(unused)]
    pub fn inner(&self) -> &I {
        &self.input
    }

    /// Returns a mutable reference to the inner source.
    #[inline]
    #[allow(unused)]
    pub fn inner_mut(&mut self) -> &mut I {
        &mut self.input
    }

    /// Returns the inner source.
    #[inline]
    #[allow(unused)]
    pub fn into_inner(self) -> I {
        self.input
    }
}

impl<I: Source> Iterator for DoneCallback<I>
where
    I: Source,
    I::Item: Sample,
{
    type Item = I::Item;

    #[inline]
    fn next(&mut self) -> Option<I::Item> {
        let next = self.input.next();
        if !self.signal_sent && next.is_none() {
            (self.signal)(());
            self.signal_sent = true;
        }
        next
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.input.size_hint()
    }
}

impl<I> Source for DoneCallback<I>
where
    I: Source,
    I::Item: Sample,
{
    #[inline]
    fn current_frame_len(&self) -> Option<usize> {
        self.input.current_frame_len()
    }

    #[inline]
    fn channels(&self) -> u16 {
        self.input.channels()
    }

    #[inline]
    fn sample_rate(&self) -> u32 {
        self.input.sample_rate()
    }

    #[inline]
    fn total_duration(&self) -> Option<Duration> {
        self.input.total_duration()
    }
}
