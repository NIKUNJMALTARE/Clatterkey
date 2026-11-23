import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { App } from "./App";

describe("App", () => {
	it("AC2: renders the app name as a heading", () => {
		render(<App />);
		expect(
			screen.getByRole("heading", { name: "Mechanical Key Sounds" }),
		).toBeInTheDocument();
	});

	it("AC2 edge: shows the line saying settings will appear here", () => {
		render(<App />);
		expect(screen.getByText("Settings will appear here.")).toBeInTheDocument();
	});
});
