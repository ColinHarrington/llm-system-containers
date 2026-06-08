//! Compile a sandbox's [`DisplayMethod`] into the concrete host recipe for viewing its GUI.
//!
//! Pure string-rendering of the steps proven in `planning/spikes/remote-display-gui.md` — no I/O,
//! so it is fully unit-testable and shared verbatim by the CLI (`llmsc display`) and the GUI.

use crate::config::{DisplayMethod, Sandbox};

/// Host/VM specifics needed to render a display recipe.
#[derive(Debug, Clone)]
pub struct DisplayCtx {
    /// SSH alias for the L1 VM (Lima) — the ProxyJump and `ssh -L` endpoint, e.g. `lima-llmsc`.
    pub vm_ssh: String,
    /// The sandbox's IP on the VM's Incus bridge (the `ssh -X` target).
    pub container_ip: String,
    /// Login user inside the sandbox (the agent's Linux user).
    pub user: String,
    /// Host/VM loopback port for the xpra proxy device + tunnel.
    pub xpra_port: u16,
    /// Client DPI for HiDPI hosts (xpra; the client value wins). 0 = client default.
    pub dpi: u16,
}

impl Default for DisplayCtx {
    fn default() -> Self {
        Self {
            vm_ssh: "lima-llmsc".to_string(),
            container_ip: "<container-ip>".to_string(),
            user: "agent".to_string(),
            xpra_port: 14500,
            dpi: 160,
        }
    }
}

/// One step of a display recipe: a short note plus the shell command to run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayStep {
    pub note: &'static str,
    pub cmd: String,
}

/// The compiled recipe for a sandbox's display method.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayPlan {
    pub method: DisplayMethod,
    pub steps: Vec<DisplayStep>,
}

/// Compile a sandbox's display recipe, or `None` when display is [`DisplayMethod::None`].
pub fn display_plan(sandbox: &Sandbox, ctx: &DisplayCtx) -> Option<DisplayPlan> {
    let steps = match sandbox.display {
        DisplayMethod::None => return None,
        DisplayMethod::Xpra => vec![
            DisplayStep {
                note: "expose the in-container xpra port to the VM (Incus proxy device)",
                cmd: format!(
                    "incus config device add {name} xpra proxy \
                     listen=tcp:127.0.0.1:{port} connect=tcp:127.0.0.1:14500 bind=host",
                    name = sandbox.name,
                    port = ctx.xpra_port,
                ),
            },
            DisplayStep {
                note: "tunnel the VM port to the host",
                cmd: format!(
                    "ssh {vm} -L {port}:127.0.0.1:{port} -N -f",
                    vm = ctx.vm_ssh,
                    port = ctx.xpra_port,
                ),
            },
            DisplayStep {
                note: "attach — native windows on the host (Ctrl-C detaches; session persists)",
                cmd: xpra_attach_cmd(ctx),
            },
        ],
        DisplayMethod::X11 => vec![DisplayStep {
            note: "forward X over ssh — apps render on the host X server (macOS: needs XQuartz)",
            cmd: format!(
                "ssh -Y -J {vm} {user}@{ip} '<app>'",
                vm = ctx.vm_ssh,
                user = ctx.user,
                ip = ctx.container_ip,
            ),
        }],
    };
    Some(DisplayPlan {
        method: sandbox.display,
        steps,
    })
}

fn xpra_attach_cmd(ctx: &DisplayCtx) -> String {
    let mut cmd = format!("xpra attach tcp://127.0.0.1:{}", ctx.xpra_port);
    if ctx.dpi > 0 {
        cmd.push_str(&format!(" --dpi={}", ctx.dpi));
    }
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Sandbox;

    fn sandbox(method: DisplayMethod) -> Sandbox {
        let mut s = Sandbox {
            name: "web-agent-01".to_string(),
            image: "images:alpine/3.21".to_string(),
            ..Default::default()
        };
        s.display = method;
        s
    }

    #[test]
    fn none_yields_no_plan() {
        assert!(display_plan(&sandbox(DisplayMethod::None), &DisplayCtx::default()).is_none());
    }

    #[test]
    fn xpra_plan_renders_proxy_tunnel_attach() {
        let plan = display_plan(&sandbox(DisplayMethod::Xpra), &DisplayCtx::default()).unwrap();
        assert_eq!(plan.method, DisplayMethod::Xpra);
        assert_eq!(plan.steps.len(), 3);
        assert!(plan.steps[0]
            .cmd
            .contains("incus config device add web-agent-01 xpra proxy"));
        assert!(plan.steps[0].cmd.contains("listen=tcp:127.0.0.1:14500"));
        assert!(plan.steps[1]
            .cmd
            .contains("ssh lima-llmsc -L 14500:127.0.0.1:14500"));
        assert_eq!(
            plan.steps[2].cmd,
            "xpra attach tcp://127.0.0.1:14500 --dpi=160"
        );
    }

    #[test]
    fn xpra_attach_omits_dpi_when_zero() {
        let ctx = DisplayCtx {
            dpi: 0,
            ..DisplayCtx::default()
        };
        let plan = display_plan(&sandbox(DisplayMethod::Xpra), &ctx).unwrap();
        assert_eq!(plan.steps[2].cmd, "xpra attach tcp://127.0.0.1:14500");
    }

    #[test]
    fn x11_plan_renders_ssh_proxyjump() {
        let ctx = DisplayCtx {
            container_ip: "10.115.43.198".to_string(),
            ..DisplayCtx::default()
        };
        let plan = display_plan(&sandbox(DisplayMethod::X11), &ctx).unwrap();
        assert_eq!(plan.method, DisplayMethod::X11);
        assert_eq!(plan.steps.len(), 1);
        assert_eq!(
            plan.steps[0].cmd,
            "ssh -Y -J lima-llmsc agent@10.115.43.198 '<app>'"
        );
    }
}
