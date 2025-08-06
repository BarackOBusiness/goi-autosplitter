# GOI Autosplitter
This is a livesplit one runtime autosplitter for use in playing Getting Over It with Bennett Foddy.

To install, download the appropriate release and configure for use with your respective livesplit one instance.
> [!NOTE]
> For linux use- depending on distro security policy- you may need to add cap_sys_ptrace capabilities to the executable hosting your livesplit one runtime.

## Contributions
If a custom map or category is desired to be supported note that there is plenty of address space left in the game state bitmask, and brainstorming is wanted on methods for dynamically loading split bounds to streamline allowing this. As of now no interest exists in creating this functionality however.

See the livesplit integration BepInEx plugin for communicating this game state information back to the autosplitter.
