interface Props {
	enabled: boolean;
	onChange(enabled: boolean): void;
}

/** The id of the switch, the first control of the settings. */
export const SOUND_SWITCH_ID = "sound-switch";

/** Turns all key sounds on or off. */
export function SoundSwitch({ enabled, onChange }: Props) {
	return (
		<div className="row">
			<label className="row-title" htmlFor={SOUND_SWITCH_ID}>
				Sound
			</label>
			<input
				id={SOUND_SWITCH_ID}
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
