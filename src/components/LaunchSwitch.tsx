interface Props {
	on: boolean;
	onChange(on: boolean): void;
}

const NOTE_ID = "launch-note";

/**
 * Has the system start the app when the user logs in. Such a start is
 * hidden: the app is in the tray and no window opens.
 */
export function LaunchSwitch({ on, onChange }: Props) {
	return (
		<section className="card">
			<div className="row row-with-note">
				<label className="row-title" htmlFor="launch-switch">
					Launch at login
				</label>
				<input
					id="launch-switch"
					className="switch"
					type="checkbox"
					role="switch"
					aria-checked={on}
					aria-describedby={NOTE_ID}
					checked={on}
					onChange={(event) => onChange(event.target.checked)}
				/>
				<p id={NOTE_ID} className="row-note">
					Starts hidden in the tray when you log in.
				</p>
			</div>
		</section>
	);
}
