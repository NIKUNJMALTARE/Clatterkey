interface Props {
	on: boolean;
	/** Says that what is playing has no key-up sounds; null when it has. */
	note: string | null;
	onChange(on: boolean): void;
}

const NOTE_ID = "key-up-note";

/**
 * Turns the sound of a key being released on or off, for every pack. With
 * a pack that has no such sounds it stays usable, and a line says so.
 */
export function KeyUpSwitch({ on, note, onChange }: Props) {
	return (
		<div className="row row-with-note">
			<label className="row-title" htmlFor="key-up-switch">
				Key-up sounds
			</label>
			<input
				id="key-up-switch"
				className="switch"
				type="checkbox"
				role="switch"
				aria-checked={on}
				aria-describedby={note ? NOTE_ID : undefined}
				checked={on}
				onChange={(event) => onChange(event.target.checked)}
			/>
			{note && (
				<p id={NOTE_ID} className="row-note">
					{note}
				</p>
			)}
		</div>
	);
}
