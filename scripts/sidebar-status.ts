/**
 * sidebar-status
 * Zellij project sidebar AI state integration for OpenCode
 *
 * Event → sidebar state mapping (mirrors Claude Code sidebar-status.sh):
 *   tool.execute.after            → active   (agent just ran a tool)
 *   session.status { busy }       → active   (agent started working)
 *   session.status { idle }       → idle     (agent finished)
 *   session.idle                  → idle     (agent went idle / Stop equivalent)
 *   permission.updated            → waiting + attention (needs permission)
 *   permission.ask hook           → waiting + attention (needs permission, backup)
 *
 * Writes /tmp/sidebar-ai/<zellij_session>/<pane_id> state files for
 * cross-session persistence, plus `zellij pipe` messages for instant
 * in-session updates.
 */

import * as fs from "node:fs/promises"
import * as path from "node:path"
import type { Plugin } from "@opencode-ai/plugin"
import type { Event } from "@opencode-ai/sdk"

// ==========================================
// ZELLIJ HELPERS
// ==========================================

function zellijSession(): string | null {
	return process.env.ZELLIJ_SESSION_NAME ?? null
}

function zellijPaneId(): string {
	return process.env.ZELLIJ_PANE_ID ?? "0"
}

async function zellijPipe(pipeName: string): Promise<void> {
	// Broadcast to all sessions via: zellij --session S pipe --name N
	try {
		const list = Bun.spawn(
			["bash", "-c", "zellij list-sessions --no-formatting 2>/dev/null | awk '{print $1}'"],
			{ stdout: "pipe", stderr: "ignore" },
		)
		const output = await new Response(list.stdout).text()
		await list.exited
		const sessions = output.split("\n").map((s) => s.trim()).filter(Boolean)
		await Promise.all(
			sessions.map(async (s) => {
				try {
					const proc = Bun.spawn(
						["bash", "-c", `zellij --session ${JSON.stringify(s)} pipe --name ${JSON.stringify(pipeName)}`],
						{ stdout: "ignore", stderr: "ignore" },
					)
					await proc.exited
				} catch {}
			}),
		)
	} catch {
		// fallback: current session only
		try {
			const proc = Bun.spawn(["bash", "-c", `zellij pipe --name ${JSON.stringify(pipeName)}`], {
				stdout: "ignore", stderr: "ignore",
			})
			await proc.exited
		} catch {}
	}
}

// ==========================================
// STATE FILE HELPERS
// ==========================================

async function writeStateFile(state: "active" | "idle" | "waiting", durationSecs = 0): Promise<void> {
	const session = zellijSession()
	if (!session) return

	const stateDir = path.join("/tmp/sidebar-ai", session)
	try {
		await fs.mkdir(stateDir, { recursive: true })
		const now = Math.floor(Date.now() / 1000)
		const content = `${state} ${now} ${durationSecs} opencode\n`
		const filePath = path.join(stateDir, zellijPaneId())
		const tmp = `${filePath}.tmp`
		await fs.writeFile(tmp, content, "utf8")
		await fs.rename(tmp, filePath)
	} catch {
		// non-fatal
	}
}

// Track when the current active run started (for duration calculation)
let activeStartSecs = 0

function elapsedAndReset(): number {
	const duration = activeStartSecs > 0
		? Math.floor(Date.now() / 1000) - activeStartSecs
		: 0
	activeStartSecs = 0
	return duration
}

// ==========================================
// PLUGIN EXPORT
// ==========================================

export const SidebarStatusPlugin: Plugin = async (_ctx) => {
	return {
		// Fires after each tool call — agent is actively working
		"tool.execute.after": async (_input: unknown) => {
			const session = zellijSession()
			if (!session) return

			if (activeStartSecs === 0) activeStartSecs = Math.floor(Date.now() / 1000)
			await writeStateFile("active")
			await zellijPipe(`sidebar::ai-active::${session}::opencode`)
			await zellijPipe(`sidebar::clear::${session}`)
		},

		// Fires before permission is granted/denied — agent needs user input
		"permission.ask": async (_input, output) => {
			const session = zellijSession()
			if (!session) return

			await writeStateFile("waiting")
			await zellijPipe(`sidebar::ai-waiting::${session}::opencode`)
			await zellijPipe(`sidebar::attention::${session}`)

			// Do not auto-allow or auto-deny — let opencode handle it
			output.status = "ask"
		},

		event: async ({ event }: { event: Event }): Promise<void> => {
			const session = zellijSession()
			if (!session) return

			switch (event.type) {
				// Agent finished — equivalent to Claude Code Stop event
				case "session.idle": {
					const duration = elapsedAndReset()
					await writeStateFile("idle", duration)
					await zellijPipe(`sidebar::ai-idle::${session}::opencode`)
					await zellijPipe(`sidebar::clear::${session}`)
					break
				}

				// Permission needed — backup to permission.ask hook
				case "permission.updated": {
					await writeStateFile("waiting")
					await zellijPipe(`sidebar::ai-waiting::${session}::opencode`)
					await zellijPipe(`sidebar::attention::${session}`)
					break
				}

				// Session status changed: busy = working, idle = done
				case "session.status": {
					type StatusEvent = { properties?: { status?: { type?: string } } }
					const props = (event as StatusEvent).properties
					const statusType = props?.status?.type

					if (statusType === "busy") {
						if (activeStartSecs === 0) activeStartSecs = Math.floor(Date.now() / 1000)
						await writeStateFile("active")
						await zellijPipe(`sidebar::ai-active::${session}::opencode`)
						await zellijPipe(`sidebar::clear::${session}`)
					} else if (statusType === "idle") {
						const duration = elapsedAndReset()
						await writeStateFile("idle", duration)
						await zellijPipe(`sidebar::ai-idle::${session}::opencode`)
						await zellijPipe(`sidebar::clear::${session}`)
					}
					break
				}
			}
		},
	}
}

export default SidebarStatusPlugin
