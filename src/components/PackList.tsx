import { useEffect, useRef, useState } from "react";
import type { PackInfo } from "../api/types";

interface Props {
	packs: PackInfo[];
	/** The id of the pack playing; null when none of them is. */
	selected: string | null;
	onChoose(id: string): void;
	/** Plays a short phrase with the sounds in force. */
	onPreview(): void;
	/** Whether the folder picker is open or an import is running. */
	importing: boolean;
	/** Opens the folder picker and imports the folder chosen. */
	onImport(): void;
	/** Removes an imported pack; resolves with whether it went. */
	onRemove(id: string): Promise<boolean>;
}

export const IMPORT_BUTTON_ID = "import-pack";

/**
 * "Author · licence", or the licence alone when there is no author; an
 * imported pack says so first.
 */
function credit(pack: PackInfo): string {
	const stated = pack.author
		? `${pack.author} · ${pack.license}`
		: pack.license;
	return pack.imported ? `Imported · ${stated}` : stated;
}

const removeButtonId = (id: string) => `remove-pack-${id}`;

/** Where focus should go once React has drawn the change that moved it. */
type FocusNext =
	| { to: "remove-button"; id: string }
	| { to: "cancel" }
	| { to: "import" };

/**
 * The packs, one chosen. Native radios in one group, so the list is a
 * single tab stop and the arrow keys move the choice. Import and Preview
 * sit by the heading and are there with or without packs. An imported pack
 * has a Remove button beside its row, outside the radio's label, which asks
 * once in the row before anything is deleted.
 */
export function PackList({
	packs,
	selected,
	onChoose,
	onPreview,
	importing,
	onImport,
	onRemove,
}: Props) {
	/** The pack whose row is asking "Remove?"; one at a time. */
	const [asking, setAsking] = useState<string | null>(null);
	/** The pack being removed, so a second press does nothing. */
	const [removing, setRemoving] = useState<string | null>(null);
	const removingRef = useRef(false);
	const [focusNext, setFocusNext] = useState<FocusNext | null>(null);
	const cancelRef = useRef<HTMLButtonElement>(null);

	// The button that had focus is gone by the time this runs, so focus is
	// put where a keyboard user would look for it.
	useEffect(() => {
		if (!focusNext) {
			return;
		}
		if (focusNext.to === "cancel") {
			cancelRef.current?.focus();
		} else if (focusNext.to === "import") {
			document.getElementById(IMPORT_BUTTON_ID)?.focus();
		} else {
			document.getElementById(removeButtonId(focusNext.id))?.focus();
		}
		setFocusNext(null);
	}, [focusNext]);

	// When the choice moves by itself (a removed pack gives way to the
	// default one, an imported pack is chosen), its row may be scrolled out
	// of the list; bring it into view. A row the user clicked is in view.
	const listRef = useRef<HTMLDivElement>(null);
	// biome-ignore lint/correctness/useExhaustiveDependencies: runs when the choice or the list changes; it reads the DOM, not these values.
	useEffect(() => {
		listRef.current
			?.querySelector("input:checked")
			?.closest(".pack-row")
			?.scrollIntoView?.({ block: "nearest" });
	}, [selected, packs]);

	const ask = (id: string) => {
		if (removingRef.current) {
			return;
		}
		setAsking(id);
		// The safe answer has focus: Enter twice must not delete a pack.
		setFocusNext({ to: "cancel" });
	};

	const cancel = (id: string) => {
		if (removingRef.current) {
			return;
		}
		setAsking(null);
		setFocusNext({ to: "remove-button", id });
	};

	const confirm = (id: string) => {
		if (removingRef.current) {
			return;
		}
		removingRef.current = true;
		setRemoving(id);
		onRemove(id)
			.catch(() => false)
			.then((gone) => {
				removingRef.current = false;
				setRemoving(null);
				setAsking(null);
				setFocusNext(gone ? { to: "import" } : { to: "remove-button", id });
			});
	};

	return (
		<section className="packs">
			<div className="section-head">
				<h2 id="packs-title" className="section-title">
					Sound pack
				</h2>
				<div className="section-actions">
					<button
						id={IMPORT_BUTTON_ID}
						type="button"
						className="button button-secondary"
						aria-disabled={importing || undefined}
						aria-busy={importing || undefined}
						onClick={() => {
							if (!importing) {
								onImport();
							}
						}}
					>
						Import pack…
					</button>
					<button
						type="button"
						className="button button-secondary"
						onClick={onPreview}
					>
						Preview
					</button>
				</div>
			</div>
			{packs.length === 0 ? (
				<p className="packs-empty">
					No sound packs were found. Reinstall the app.
				</p>
			) : (
				<div
					ref={listRef}
					className="pack-list"
					role="radiogroup"
					aria-labelledby="packs-title"
				>
					{packs.map((pack) => (
						<div key={pack.id} className="pack-row">
							<label className="pack">
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
							{pack.imported &&
								(asking === pack.id ? (
									// biome-ignore lint/a11y/useSemanticElements: a fieldset would put a second border and legend in the row; this is a group of two buttons.
									<div
										className="pack-actions pack-question"
										role="group"
										aria-label={`Remove ${pack.name}?`}
										onKeyDown={(event) => {
											if (event.key === "Escape") {
												event.stopPropagation();
												cancel(pack.id);
											}
										}}
									>
										<span className="pack-question-text" aria-hidden="true">
											Remove?
										</span>
										<button
											type="button"
											className="button button-small button-danger"
											aria-disabled={removing === pack.id || undefined}
											onClick={() => confirm(pack.id)}
										>
											Remove
										</button>
										<button
											ref={cancelRef}
											type="button"
											className="button button-small button-secondary"
											aria-disabled={removing === pack.id || undefined}
											onClick={() => cancel(pack.id)}
										>
											Cancel
										</button>
									</div>
								) : (
									<div className="pack-actions">
										<button
											id={removeButtonId(pack.id)}
											type="button"
											className="button button-small button-secondary"
											aria-label={`Remove ${pack.name}`}
											onClick={() => ask(pack.id)}
										>
											Remove
										</button>
									</div>
								))}
						</div>
					))}
				</div>
			)}
		</section>
	);
}
