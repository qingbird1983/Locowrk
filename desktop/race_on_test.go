//go:build race

package desktop

// raceBuild reports whether the race detector instruments this test binary.
// Timing assertions loosen under it: instrumentation dominates the frame cost.
const raceBuild = true
