// Fault fixture: the factory throws during initialize. The shim reports a
// factory_error diagnostic, and the host must disable the extension instead of
// leaving a healthy process with no tools.
export default function factory() {
  throw new Error("fixture factory exploded");
}
