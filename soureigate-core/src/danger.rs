/// Checks if a shell command is potentially dangerous
pub fn is_dangerous_command(cmd: &str) -> bool {
    let lower = cmd.to_lowercase();
    let patterns = [
        "reboot", "shutdown", "poweroff", "halt",
        "rm -rf", "rm -r /", "rmdir",
        "mkfs", "dd if=", "fdisk",
        "iptables -f", "iptables -x",
        "systemctl stop", "systemctl disable",
        "kill -9", "killall",
        "chmod 777", "chmod -r",
        "> /dev/sda", "> /dev/null",
    ];
    patterns.iter().any(|p| lower.contains(p))
}
