import { Slider } from "./Slider";

interface Props {
	/** 0 to 30. */
	percent: number;
	onChange(percent: number): void;
	/** The slider was let go: send the value without waiting. */
	onRelease(): void;
}

/**
 * How far each sound's pitch may stray, 0 to 30%. At 0 every sound plays
 * as recorded, and the value reads "Off".
 */
export function PitchSlider({ percent, onChange, onRelease }: Props) {
	return (
		<Slider
			id="pitch-slider"
			label="Pitch variation"
			value={percent}
			max={30}
			text={percent === 0 ? "Off" : `${percent}%`}
			onChange={onChange}
			onRelease={onRelease}
		/>
	);
}
