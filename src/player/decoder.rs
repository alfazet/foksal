use anyhow::Result;

// has its own async task + a separate OS thread for doing the decoding
// has a double-directional connection with the rest of the player
// player->decoder: Pause, Resume, Stop, Enqueue(uri)

// how to transport the samples from the decoder to the resampler?
// AudioFrame = collection of ~100ms worth of samples
// from local: cbeam_chan::<AudioFrame> to the ResamplerContoller, ResamplerContoller is started by LocalController
// from proxy: cbeam_chan::<AudioFrame> to an adapter (started by the HeadlessController, the
// sends the samples through a secondary connection to an adapter on the Proxy side (that adapter
// is started by the ProxyController) and from there to the resampler by another cbeam_chan)

pub enum DecoderRequest {
    //
}
