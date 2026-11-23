export interface Notice {
	id: string;
	/** An error interrupts a screen reader; a warning waits its turn. */
	tone: "error" | "warning";
	text: string;
	/** Absent for notices that go away by themselves. */
	onDismiss?: () => void;
	/** A button that does something about what the notice says. */
	action?: { label: string; onAction: () => void };
}

/**
 * A dismissed notice takes its button with it. If the button had focus, it
 * goes to the next thing a Tab would have reached, so a keyboard user keeps
 * their place: the next notice's button, or the first control below.
 */
function keepFocus(button: HTMLButtonElement) {
	if (document.activeElement !== button) {
		return;
	}
	const strip = button.closest(".notices");
	const buttons = Array.from(
		strip?.querySelectorAll<HTMLButtonElement>(".notice-dismiss") ?? [],
	);
	const next =
		buttons[buttons.indexOf(button) + 1] ??
		strip?.parentElement?.querySelector<HTMLElement>(
			".notices ~ * input, .notices ~ * button",
		);
	next?.focus();
}

/** A strip of short messages at the top of the window, worst first. */
export function Notices({ notices }: { notices: Notice[] }) {
	if (notices.length === 0) {
		return null;
	}
	return (
		<div className="notices">
			{notices.map((notice) => (
				<div
					key={notice.id}
					className={`notice notice-${notice.tone}`}
					role={notice.tone === "error" ? "alert" : "status"}
				>
					<p>{notice.text}</p>
					{notice.action && (
						<button
							type="button"
							className="button notice-action"
							onClick={notice.action.onAction}
						>
							{notice.action.label}
						</button>
					)}
					{notice.onDismiss && (
						<button
							type="button"
							className="notice-dismiss"
							aria-label="Dismiss"
							onClick={(event) => {
								keepFocus(event.currentTarget);
								notice.onDismiss?.();
							}}
						>
							<span aria-hidden="true">×</span>
						</button>
					)}
				</div>
			))}
		</div>
	);
}
