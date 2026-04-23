# Google Vimscript Style Guide Frozen Snapshot

Source: https://google.github.io/styleguide/vimscriptguide.xml
Snapshot date: 2026-04-24
Attribution: Copied from the Google Style Guides project for Maestro bundled setup use.
License: Creative Commons Attribution 3.0 (https://creativecommons.org/licenses/by/3.0/)

---

# Google Vimscript Style Guide

Revision 1.1

Nate Soares\
Artemis Sparks\
David Barnett\

<div style="margin-left: 50%; font-size: 75%;">

Each style point has a summary for which additional information is available by toggling the accompanying arrow button that looks this way: <span class="showhide_button" style="margin-left: 0; float: none">▶</span>. You may toggle all summaries with the big arrow button:

<div style=" font-size: larger; margin-left: +2em;">

<span id="show_hide_all_button" class="showhide_button" style="font-size: 180%; float: none" onclick="javascript:ShowHideAll()">▶</span> Toggle all summaries

</div>

</div>

<div class="toc">

<div class="toc_title">

Table of Contents

</div>

<table>
<colgroup>
<col style="width: 50%" />
<col style="width: 50%" />
</colgroup>
<tbody>
<tr data-valign="top">
<td><div class="toc_category">
<a href="#Portability">Portability</a>
</div></td>
<td><div class="toc_stylepoint">
<span style="padding-right: 1em; white-space:nowrap;"><a href="#Strings">Strings</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#Matching_Strings">Matching Strings</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#Regular_Expressions">Regular Expressions</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#Dangerous_commands">Dangerous commands</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#Fragile_commands">Fragile commands</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#Catching_Exceptions">Catching Exceptions</a></span>
</div></td>
</tr>
<tr data-valign="top">
<td><div class="toc_category">
<a href="#General_Guidelines">General Guidelines</a>
</div></td>
<td><div class="toc_stylepoint">
<span style="padding-right: 1em; white-space:nowrap;"><a href="#Messaging">Messaging</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#Type_checking">Type checking</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#Python">Python</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#Other_Languages">Other Languages</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#Boilerplate">Boilerplate</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#Plugin_layout">Plugin layout</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#Functions">Functions</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#Commands">Commands</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#Autocommands">Autocommands</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#Mappings">Mappings</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#Settings">Settings</a></span>
</div></td>
</tr>
<tr data-valign="top">
<td><div class="toc_category">
<a href="#Style">Style</a>
</div></td>
<td><div class="toc_stylepoint">
<span style="padding-right: 1em; white-space:nowrap;"><a href="#Whitespace">Whitespace</a></span> <span style="padding-right: 1em; white-space:nowrap;"><a href="#Naming">Naming</a></span>
</div></td>
</tr>
</tbody>
</table>

</div>

<div>

## Background

This is a casual version of the vimscript style guide, because vimscript is a casual language. When submitting vim plugin code, you must adhere to these rules. For clarifications, justifications, and explanations about the finer points of vimscript, please refer to the [heavy guide](vimscriptfull.xml).

</div>

<div>

## Portability

It's hard to get vimscript right. Many commands depend upon the user's settings. By following these guidelines, you can hope to make your scripts portable.

<div>

### <span id="Strings">Strings</span>

<span id="link-Strings__button" class="link_button"> [link](?showone=Strings#Strings) </span><span id="Strings__button" class="showhide_button" onclick="javascript:ShowHideByName('Strings')">▶</span>

<div style="display:inline;">

Prefer single quoted strings

</div>

<div>

<div id="Strings__body" class="stylepoint_body" style="display: none">

Double quoted strings are semantically different in vimscript, and you probably don't want them (they break regexes).

Use double quoted strings when you need an escape sequence (such as `"\n"`) or if you know it doesn't matter and you need to embed single quotes.

</div>

</div>

</div>

<div>

### <span id="Matching_Strings">Matching Strings</span>

<span id="link-Matching_Strings__button" class="link_button"> [link](?showone=Matching_Strings#Matching_Strings) </span><span id="Matching_Strings__button" class="showhide_button" onclick="javascript:ShowHideByName('Matching_Strings')">▶</span>

<div style="display:inline;">

Use the `=~#` or `=~?` operator families over the `=~` family.

</div>

<div>

<div id="Matching_Strings__body" class="stylepoint_body" style="display: none">

The matching behavior depends upon the user's ignorecase and smartcase settings and on whether you compare them with the `=~`, `=~#`, or `=~?` family of operators. Use the `=~#` and `=~?` operator families explicitly when comparing strings unless you explicitly need to honor the user's case sensitivity settings.

</div>

</div>

</div>

<div>

### <span id="Regular_Expressions">Regular Expressions</span>

<span id="link-Regular_Expressions__button" class="link_button"> [link](?showone=Regular_Expressions#Regular_Expressions) </span><span id="Regular_Expressions__button" class="showhide_button" onclick="javascript:ShowHideByName('Regular_Expressions')">▶</span>

<div style="display:inline;">

Prefix all regexes with `\m\C`.

</div>

<div>

<div id="Regular_Expressions__body" class="stylepoint_body" style="display: none">

In addition to the case sensitivity settings, regex behavior depends upon the user's nomagic setting. To make regexes act like nomagic and noignorecase are set, prepend all regexes with `\m\C`.

You are welcome to use other magic levels (`\v`) and case sensitivities (`\c`) so long as they are intentional and explicit.

</div>

</div>

</div>

<div>

### <span id="Dangerous_commands">Dangerous commands</span>

<span id="link-Dangerous_commands__button" class="link_button"> [link](?showone=Dangerous_commands#Dangerous_commands) </span><span id="Dangerous_commands__button" class="showhide_button" onclick="javascript:ShowHideByName('Dangerous_commands')">▶</span>

<div style="display:inline;">

Avoid commands with unintended side effects.

</div>

<div>

<div id="Dangerous_commands__body" class="stylepoint_body" style="display: none">

Avoid using `:s[ubstitute]` as it moves the cursor and prints error messages. Prefer functions (such as `search()`) better suited to scripts.

For many vim commands, functions exist that do the same thing with fewer side effects. See `:help functions()` for a list of built-in functions.

</div>

</div>

</div>

<div>

### <span id="Fragile_commands">Fragile commands</span>

<span id="link-Fragile_commands__button" class="link_button"> [link](?showone=Fragile_commands#Fragile_commands) </span><span id="Fragile_commands__button" class="showhide_button" onclick="javascript:ShowHideByName('Fragile_commands')">▶</span>

<div style="display:inline;">

Avoid commands that rely on user settings.

</div>

<div>

<div id="Fragile_commands__body" class="stylepoint_body" style="display: none">

Always use `normal!` instead of `normal`. The latter depends upon the user's key mappings and could do anything.

Avoid `:s[ubstitute]`, as its behavior depends upon a number of local settings.

The same applies to other commands not listed here.

</div>

</div>

</div>

<div>

### <span id="Catching_Exceptions">Catching Exceptions</span>

<span id="link-Catching_Exceptions__button" class="link_button"> [link](?showone=Catching_Exceptions#Catching_Exceptions) </span><span id="Catching_Exceptions__button" class="showhide_button" onclick="javascript:ShowHideByName('Catching_Exceptions')">▶</span>

<div style="display:inline;">

Match error codes, not error text.

</div>

<div>

<div id="Catching_Exceptions__body" class="stylepoint_body" style="display: none">

Error text may be locale dependent.

</div>

</div>

</div>

</div>

<div>

## General Guidelines

<div>

### <span id="Messaging">Messaging</span>

<span id="link-Messaging__button" class="link_button"> [link](?showone=Messaging#Messaging) </span><span id="Messaging__button" class="showhide_button" onclick="javascript:ShowHideByName('Messaging')">▶</span>

<div style="display:inline;">

Message the user infrequently.

</div>

<div>

<div id="Messaging__body" class="stylepoint_body" style="display: none">

Loud scripts are annoying. Message the user only when:

- A long-running process has kicked off.
- An error has occurred.

</div>

</div>

</div>

<div>

### <span id="Type_checking">Type checking</span>

<span id="link-Type_checking__button" class="link_button"> [link](?showone=Type_checking#Type_checking) </span><span id="Type_checking__button" class="showhide_button" onclick="javascript:ShowHideByName('Type_checking')">▶</span>

<div style="display:inline;">

Use strict and explicit checks where possible.

</div>

<div>

<div id="Type_checking__body" class="stylepoint_body" style="display: none">

Vimscript has unsafe, unintuitive behavior when dealing with some types. For instance, `0 == 'foo'` evaluates to true.

Use strict comparison operators where possible. When comparing against a string literal, use the `is#` operator. Otherwise, prefer `maktaba#value#IsEqual` or check `type()` explicitly.

Check variable types explicitly before using them. Use functions from `maktaba#ensure`, or check `maktaba#value` or `type()` and throw your own errors.

Use `:unlet` for variables that may change types, particularly those assigned inside loops.

</div>

</div>

</div>

<div>

### <span id="Python">Python</span>

<span id="link-Python__button" class="link_button"> [link](?showone=Python#Python) </span><span id="Python__button" class="showhide_button" onclick="javascript:ShowHideByName('Python')">▶</span>

<div style="display:inline;">

Use sparingly.

</div>

<div>

<div id="Python__body" class="stylepoint_body" style="display: none">

Use python only when it provides critical functionality, for example when writing threaded code.

</div>

</div>

</div>

<div>

### <span id="Other_Languages">Other Languages</span>

<span id="link-Other_Languages__button" class="link_button"> [link](?showone=Other_Languages#Other_Languages) </span><span id="Other_Languages__button" class="showhide_button" onclick="javascript:ShowHideByName('Other_Languages')">▶</span>

<div style="display:inline;">

Use vimscript instead.

</div>

<div>

<div id="Other_Languages__body" class="stylepoint_body" style="display: none">

Avoid using other scripting languages such as ruby and lua. We can not guarantee that the end user's vim has been compiled with support for non-vimscript languages.

</div>

</div>

</div>

<div>

### <span id="Boilerplate">Boilerplate</span>

<span id="link-Boilerplate__button" class="link_button"> [link](?showone=Boilerplate#Boilerplate) </span><span id="Boilerplate__button" class="showhide_button" onclick="javascript:ShowHideByName('Boilerplate')">▶</span>

<div style="display:inline;">

Use [maktaba](https://github.com/google/maktaba).

</div>

<div>

<div id="Boilerplate__body" class="stylepoint_body" style="display: none">

maktaba removes boilerplate, including:

- Plugin creation
- Error handling
- Dependency checking

</div>

</div>

</div>

<div>

### <span id="Plugin_layout">Plugin layout</span>

<span id="link-Plugin_layout__button" class="link_button"> [link](?showone=Plugin_layout#Plugin_layout) </span><span id="Plugin_layout__button" class="showhide_button" onclick="javascript:ShowHideByName('Plugin_layout')">▶</span>

<div style="display:inline;">

Organize functionality into modular plugins

</div>

<div>

<div id="Plugin_layout__body" class="stylepoint_body" style="display: none">

Group your functionality as a plugin, unified in one directory (or code repository) which shares your plugin's name (with a "vim-" prefix or ".vim" suffix if desired). It should be split into plugin/, autoload/, etc. subdirectories as necessary, and it should declare metadata in the addon-info.json format (see the [VAM documentation](https://github.com/MarcWeber/vim-addon-manager/blob/master/doc/vim-addon-manager-additional-documentation.txt) for details).

</div>

</div>

</div>

<div>

### <span id="Functions">Functions</span>

<span id="link-Functions__button" class="link_button"> [link](?showone=Functions#Functions) </span><span id="Functions__button" class="showhide_button" onclick="javascript:ShowHideByName('Functions')">▶</span>

<div style="display:inline;">

In the autoload/ directory, defined with `[!]` and `[abort]`.

</div>

<div>

<div id="Functions__body" class="stylepoint_body" style="display: none">

Autoloading allows functions to be loaded on demand, which makes startuptime faster and enforces function namespacing.

Script-local functions are welcome, but should also live in autoload/ and be called by autoloaded functions.

Non-library plugins should expose commands instead of functions. Command logic should be extracted into functions and autoloaded.

`[!]` allows developers to reload their functions without complaint.

`[abort]` forces the function to halt when it encounters an error.

</div>

</div>

</div>

<div>

### <span id="Commands">Commands</span>

<span id="link-Commands__button" class="link_button"> [link](?showone=Commands#Commands) </span><span id="Commands__button" class="showhide_button" onclick="javascript:ShowHideByName('Commands')">▶</span>

<div style="display:inline;">

In the plugin/commands.vim or under the ftplugin/ directory, defined without `[!]`.

</div>

<div>

<div id="Commands__body" class="stylepoint_body" style="display: none">

General commands go in `plugin/commands.vim`. Filetype-specific commands go in `ftplugin/`.

Excluding `[!]` prevents your plugin from silently clobbering existing commands. Command conflicts should be resolved by the user.

</div>

</div>

</div>

<div>

### <span id="Autocommands">Autocommands</span>

<span id="link-Autocommands__button" class="link_button"> [link](?showone=Autocommands#Autocommands) </span><span id="Autocommands__button" class="showhide_button" onclick="javascript:ShowHideByName('Autocommands')">▶</span>

<div style="display:inline;">

Place them in plugin/autocmds.vim, within augroups.

</div>

<div>

<div id="Autocommands__body" class="stylepoint_body" style="display: none">

Place all autocommands in augroups.

The augroup name should be unique. It should either be, or be prefixed with, the plugin name.

Clear the augroup with `autocmd!` before defining new autocommands in the augroup. This makes your plugin re-entrable.

</div>

</div>

</div>

<div>

### <span id="Mappings">Mappings</span>

<span id="link-Mappings__button" class="link_button"> [link](?showone=Mappings#Mappings) </span><span id="Mappings__button" class="showhide_button" onclick="javascript:ShowHideByName('Mappings')">▶</span>

<div style="display:inline;">

Place them in `plugin/mappings.vim`, using `maktaba#plugin#MapPrefix` to get a prefix.

</div>

<div>

<div id="Mappings__body" class="stylepoint_body" style="display: none">

All key mappings should be defined in `plugin/mappings.vim`.

Partial mappings (see :help using-\<Plug\>.) should be defined in `plugin/plugs.vim`.

</div>

</div>

</div>

<div>

### <span id="Settings">Settings</span>

<span id="link-Settings__button" class="link_button"> [link](?showone=Settings#Settings) </span><span id="Settings__button" class="showhide_button" onclick="javascript:ShowHideByName('Settings')">▶</span>

<div style="display:inline;">

Change settings locally

</div>

<div>

<div id="Settings__body" class="stylepoint_body" style="display: none">

Use `:setlocal` and `&l:` instead of `:set` and `&` unless you have explicit reason to do otherwise.

</div>

</div>

</div>

</div>

<div>

## Style

Follow google style conventions. When in doubt, treat vimscript style like python style.

<div>

### <span id="Whitespace">Whitespace</span>

<span id="link-Whitespace__button" class="link_button"> [link](?showone=Whitespace#Whitespace) </span><span id="Whitespace__button" class="showhide_button" onclick="javascript:ShowHideByName('Whitespace')">▶</span>

<div style="display:inline;">

Similar to python.\
\

</div>

<div>

<div id="Whitespace__body" class="stylepoint_body" style="display: none">

- Use two spaces for indents

- Do not use tabs

- Use spaces around operators

  This does not apply to arguments to commands.

  <div>

      let s:variable = "concatenated " . "strings"
      command -range=% MyCommand

  </div>

- Do not introduce trailing whitespace

  You need not go out of your way to remove it.

  Trailing whitespace is allowed in mappings which prep commands for user input, such as "`noremap <leader>gf :grep -f `".

- Restrict lines to 80 columns wide

- Indent continued lines by four spaces

- Do not align arguments of commands
  <div>

  ``` badcode
  command -bang MyCommand  call myplugin#foo()
  command       MyCommand2 call myplugin#bar()
  ```

  </div>

  <div>

      command -bang MyCommand call myplugin#foo()
      command MyCommand2 call myplugin#bar()

  </div>

</div>

</div>

</div>

<div>

### <span id="Naming">Naming</span>

<span id="link-Naming__button" class="link_button"> [link](?showone=Naming#Naming) </span><span id="Naming__button" class="showhide_button" onclick="javascript:ShowHideByName('Naming')">▶</span>

<div style="display:inline;">

In general, use `plugin-names-like-this`, `FunctionNamesLikeThis`, `CommandNamesLikeThis`, `augroup_names_like_this`, `variable_names_like_this`.

Always prefix variables with their scope.

</div>

<div>

<div id="Naming__body" class="stylepoint_body" style="display: none">

<span class="stylepoint_subsection">plugin-names-like-this</span>

Keep them short and sweet.

<span class="stylepoint_subsection">FunctionNamesLikeThis</span>

Prefix script-local functions with `s:`

Autoloaded functions may not have a scope prefix.

Do not create global functions. Use autoloaded functions instead.

<span class="stylepoint_subsection">CommandNamesLikeThis</span>

Prefer succinct command names over common command prefixes.

<span class="stylepoint_subsection">variable_names_like_this</span>

Augroup names count as variables for naming purposes.

<span class="stylepoint_subsection">Prefix all variables with their scope.</span>

- Global variables with `g:`
- Script-local variables with `s:`
- Function arguments with `a:`
- Function-local variables with `l:`
- Vim-predefined variables with `v:`
- Buffer-local variables with `b:`

`g:`, `s:`, and `a:` must always be used.

`b:` changes the variable semantics; use it when you want buffer-local semantics.

`l:` and `v:` should be used for consistency, future proofing, and to avoid subtle bugs. They are not strictly required. Add them in new code but don’t go out of your way to add them elsewhere.

</div>

</div>

</div>

</div>

Revision 1.1

Nate Soares\
Artemis Sparks\
David Barnett\
