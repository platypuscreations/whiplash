//! A simple linear-interpolation resampler for stereo audio.
//!
//! Only used when the sound device can't run at a track's own sample rate,
//! which is rare: CoreAudio, PipeWire and PulseAudio accept common rates
//! directly.

pub struct Resampler {
    /// Input frames to advance per output frame (input rate / output rate).
    step: f64,
    /// Read position, in frames, where 0 is `previous` and 1 is the first
    /// frame of the next input chunk.
    position: f64,
    /// The last frame of the previous chunk, so chunks join smoothly.
    previous: [f32; 2],
}

impl Resampler {
    pub fn new(input_rate: u32, output_rate: u32) -> Resampler {
        // Start on the first real frame rather than the empty `previous`.
        Resampler { step: input_rate as f64 / output_rate as f64, position: 1.0, previous: [0.0; 2] }
    }

    /// Resamples interleaved stereo `input`, appending the result to `output`.
    pub fn process(&mut self, input: &[f32], output: &mut Vec<f32>) {
        let frames = input.len() / 2;
        if frames == 0 {
            return;
        }
        // Frame 0 is the saved previous frame; frame i (i >= 1) is input frame i - 1.
        let frame = |i: usize| -> [f32; 2] {
            if i == 0 { self.previous } else { [input[2 * (i - 1)], input[2 * (i - 1) + 1]] }
        };

        while self.position < frames as f64 {
            let index = self.position as usize;
            let t = (self.position - index as f64) as f32;
            let (a, b) = (frame(index), frame(index + 1));
            output.push(a[0] + (b[0] - a[0]) * t);
            output.push(a[1] + (b[1] - a[1]) * t);
            self.position += self.step;
        }

        self.position -= frames as f64;
        self.previous = [input[2 * frames - 2], input[2 * frames - 1]];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_length_follows_the_rate_ratio() {
        let mut resampler = Resampler::new(44_100, 48_000);
        let mut output = Vec::new();
        // One second of audio, fed in uneven chunks.
        let input = vec![0.5; 44_100 * 2];
        for chunk in input.chunks(2 * 1000 + 2) {
            resampler.process(chunk, &mut output);
        }
        let frames = output.len() / 2;
        assert!((47_990..=48_010).contains(&frames), "got {frames} frames");
        // A constant signal stays constant.
        assert!(output.iter().all(|&s| (s - 0.5).abs() < 1e-6));
    }

    #[test]
    fn interpolates_between_frames() {
        let mut resampler = Resampler::new(1, 2);
        let mut output = Vec::new();
        resampler.process(&[1.0, -1.0, 3.0, -3.0], &mut output);
        assert_eq!(output, vec![1.0, -1.0, 2.0, -2.0]);
        // The last input frame is carried over and blended into the next chunk.
        output.clear();
        resampler.process(&[5.0, -5.0], &mut output);
        assert_eq!(output, vec![3.0, -3.0, 4.0, -4.0]);
    }
}
