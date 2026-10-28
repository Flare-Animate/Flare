// Frame-script example. Paste into Window > Actions on a keyframe.
// Animate HTML5 Canvas style: `this` is the timeline, frames are 0-based.
if (!this.loops) this.loops = 0;
this.loops++;
trace("loop", this.loops);
if (this.loops >= 3) this.stop();          // stop after three passes
else this.gotoAndPlay("start");             // jump to the frame labelled "start"
