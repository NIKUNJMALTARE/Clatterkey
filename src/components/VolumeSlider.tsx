import type { CSSProperties } from "react";

interface Props {
	/** 0 to 100. */
	percent: number;
	onChange(percent: number): void;
	/** The slider was let go: send the value without waiting. */
	onRelease(): void;
}

/**
 * The volume, 0 to 100. A native range input, so arrows move by 1, Page Up
 * and Page Down by 10, and Home and End go to the ends.
 */
export function VolumeSlider({ percent, onChange, onRelease }: Props) {
	const text = `${percent}%`;
	return (
		<div className="row">
			<label className="row-title" htmlFor="volume-slider">
				Volume
			</label>
			<input
				id="volume-slider"
				className="slider"
				type="range"
				min={0}
				max={100}
				step={1}
				value={percent}
				aria-valuetext={text}
				style={{ "--fill": text } as CSSProperties}
				onChange={(event) => onChange(Number(event.target.value))}
				onPointerUp={onRelease}
				onKeyUp={onRelease}
				onBlur={onRelease}
			/>
			<output className="slider-value" htmlFor="volume-slider">
				{text}
			</output>
		</div>
	);
}
