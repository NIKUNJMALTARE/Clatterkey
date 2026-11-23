import { Slider } from "./Slider";

interface Props {
	/** 0 to 100. */
	percent: number;
	onChange(percent: number): void;
	/** The slider was let go: send the value without waiting. */
	onRelease(): void;
}

/** The volume, 0 to 100. */
export function VolumeSlider({ percent, onChange, onRelease }: Props) {
	return (
		<Slider
			id="volume-slider"
			label="Volume"
			value={percent}
			max={100}
			text={`${percent}%`}
			onChange={onChange}
			onRelease={onRelease}
		/>
	);
}
