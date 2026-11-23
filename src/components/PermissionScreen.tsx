import { useEffect, useRef, useState } from "react";

interface Props {
	/** macOS has granted the permission but the app must be restarted. */
	restart: boolean;
	/** Rejects if System Settings could not be opened. */
	onOpen(): Promise<void>;
	onLater(): void;
	onRestart(): void;
}

/** The id of the heading, which takes focus when the screen appears. */
export const PERMISSION_HEADING_ID = "permission-heading";

/**
 * Shown in place of the settings while macOS does not let the app hear
 * keys: why, where to allow it, and what happens next.
 */
export function PermissionScreen({
	restart,
	onOpen,
	onLater,
	onRestart,
}: Props) {
	const [asked, setAsked] = useState<"no" | "waiting" | "failed">("no");
	// The button can be pressed again before the last press has answered;
	// only the latest answer is shown.
	const latest = useRef(0);

	// When the screen changes to or from its restart form, the buttons that
	// were there are gone, and focus with them. It goes to the heading.
	const heading = useRef<HTMLHeadingElement>(null);
	const form = useRef(restart);
	useEffect(() => {
		if (form.current === restart) {
			return;
		}
		form.current = restart;
		heading.current?.focus();
	}, [restart]);

	const open = () => {
		latest.current += 1;
		const press = latest.current;
		setAsked("waiting");
		onOpen().catch(() => {
			if (press === latest.current) {
				setAsked("failed");
			}
		});
	};

	return (
		<section
			className="card permission"
			aria-labelledby={PERMISSION_HEADING_ID}
		>
			<h2
				id={PERMISSION_HEADING_ID}
				ref={heading}
				className="section-title"
				tabIndex={-1}
			>
				Let Mechanical Key Sounds hear your keys
			</h2>
			{/*
			 * One status for both forms of the screen, always in the page, so
			 * that its text is announced when it comes or changes. Without
			 * the restart it is drawn below the buttons (see app.css).
			 */}
			<p
				role="status"
				className={restart ? "permission-status" : "permission-progress"}
			>
				{restart
					? "Permission granted. Restart Mechanical Key Sounds to finish."
					: asked === "waiting"
						? "Waiting for permission…"
						: ""}
			</p>
			{restart ? (
				<>
					<div className="permission-actions">
						<button type="button" className="button" onClick={onRestart}>
							Restart now
						</button>
					</div>
				</>
			) : (
				<>
					<p>
						macOS only tells an app that a key was pressed if you allow it under
						Input Monitoring. Until you do, the app is silent.
					</p>
					<p>
						The app only notices that a key went down, so it can play a sound.
						It never records, stores or sends what you type.
					</p>
					<ol className="permission-steps">
						<li>Click Open System Settings.</li>
						<li>Turn on Mechanical Key Sounds in the Input Monitoring list.</li>
						<li>Come back here. The sound starts by itself.</li>
					</ol>
					<div className="permission-actions">
						<button type="button" className="button" onClick={open}>
							Open System Settings
						</button>
						<button
							type="button"
							className="button button-secondary"
							onClick={onLater}
						>
							Set up later
						</button>
					</div>
					{asked === "failed" && (
						<p role="alert" className="permission-failed">
							System Settings could not be opened. Open it yourself: Privacy
							&amp; Security, then Input Monitoring.
						</p>
					)}
				</>
			)}
		</section>
	);
}
