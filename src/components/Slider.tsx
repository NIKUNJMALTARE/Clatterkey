import type { CSSProperties, KeyboardEvent } from "react";

interface Props {
	id: string;
	label: string;
	/** 0 to `max`, a whole number. */
	value: number;
	max: number;
	/** The value as shown beside the slider and read out: "80%", "Off". */
	text: string;
	onChange(value: number): void;
	/** The slider was let go: send the value without waiting. */
	onRelease(): void;
}

/** How far Page Up and Page Down move a slider. */
const PAGE_STEP = 10;

/**
 * A labelled slider with its value beside it. A native range input, so
 * arrows move by 1 and Home and End go to the ends. Page Up and Page Down
 * move by 10: left to the browser they move by a share of the range, which
 * differs between sliders and between engines.
 */
export function Slider({
	id,
	label,
	value,
	max,
	text,
	onChange,
	onRelease,
}: Props) {
	// How much of the track is filled, whatever the slider's top is.
	const fill = `${max > 0 ? (value / max) * 100 : 0}%`;
	const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
		if (event.key !== "PageUp" && event.key !== "PageDown") {
			return;
		}
		event.preventDefault();
		const moved = value + (event.key === "PageUp" ? PAGE_STEP : -PAGE_STEP);
		const next = Math.min(max, Math.max(0, moved));
		if (next !== value) {
			onChange(next);
		}
	};
	return (
		<div className="row row-slider">
			<label className="row-title" htmlFor={id}>
				{label}
			</label>
			<input
				id={id}
				className="slider"
				type="range"
				min={0}
				max={max}
				step={1}
				value={value}
				aria-valuetext={text}
				style={{ "--fill": fill } as CSSProperties}
				onChange={(event) => onChange(Number(event.target.value))}
				onKeyDown={onKeyDown}
				onPointerUp={onRelease}
				onKeyUp={onRelease}
				onBlur={onRelease}
			/>
			<output className="slider-value" htmlFor={id}>
				{text}
			</output>
		</div>
	);
}
