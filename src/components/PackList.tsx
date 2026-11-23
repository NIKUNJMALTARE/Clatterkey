import type { PackInfo } from "../api/types";

interface Props {
	packs: PackInfo[];
	/** The id of the pack playing; null when none of them is. */
	selected: string | null;
	onChoose(id: string): void;
	/** Plays a short phrase with the sounds in force. */
	onPreview(): void;
}

/** "Author · licence", or the licence alone when there is no author. */
function credit(pack: PackInfo): string {
	return pack.author ? `${pack.author} · ${pack.license}` : pack.license;
}

/**
 * The packs, one chosen. Native radios in one group, so the list is a
 * single tab stop and the arrow keys move the choice. Preview sits by the
 * heading and is there with or without packs: it plays whatever is playing.
 */
export function PackList({ packs, selected, onChoose, onPreview }: Props) {
	return (
		<section className="packs">
			<div className="section-head">
				<h2 id="packs-title" className="section-title">
					Sound pack
				</h2>
				<button
					type="button"
					className="button button-secondary"
					onClick={onPreview}
				>
					Preview
				</button>
			</div>
			{packs.length === 0 ? (
				<p className="packs-empty">
					No sound packs were found. Reinstall the app.
				</p>
			) : (
				<div
					className="pack-list"
					role="radiogroup"
					aria-labelledby="packs-title"
				>
					{packs.map((pack) => (
						<label key={pack.id} className="pack">
							<input
								type="radio"
								name="sound-pack"
								value={pack.id}
								checked={pack.id === selected}
								onChange={() => onChoose(pack.id)}
							/>
							<span className="pack-text">
								<span className="pack-name">{pack.name}</span>
								<span className="pack-credit">{credit(pack)}</span>
							</span>
						</label>
					))}
				</div>
			)}
		</section>
	);
}
