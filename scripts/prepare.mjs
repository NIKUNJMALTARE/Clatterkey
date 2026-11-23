// Run by `npm install`. Points git at .githooks/ so the pre-push hook is
// active. Outside a git checkout (for example a tarball) there is nothing to
// configure, so a failure here is ignored. Written in Node, not shell, so it
// behaves the same on Windows.
import { execSync } from "node:child_process";

try {
	execSync("git config core.hooksPath .githooks", { stdio: "ignore" });
} catch {
	// Not a git checkout, or git is not installed.
}
