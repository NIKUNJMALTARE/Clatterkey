interface Props {
	enabled: boolean;
	onChange(enabled: boolean): void;
}

/** Turns all key sounds on or off. */
export function SoundSwitch({ enabled, onChange }: Props) {
	return (
		<div className="row">
			<label className="row-title" htmlFor="sound-switch">
				Sound
			</label>
			<input
				id="sound-switch"
				className="switch"
				type="checkbox"
				role="switch"
				aria-checked={enabled}
				checked={enabled}
				onChange={(event) => onChange(event.target.checked)}
			/>
		</div>
	);
}
