# PROJECT: Remote Manager for Windows SysAdmin

## Goal

Build a lightweight Windows desktop application to manage SSH and RDP connections for infrastructure administrators.

The application should be a free internal alternative to mRemoteNG with a modern UI and better SSH experience.

Target users:
- System Administrators
- DevOps Engineers
- Infrastructure Engineers

Target OS:
- Windows 10
- Windows 11
- Windows Server 2019/2022

---

# Technology Stack

Frontend:
- React
- TypeScript
- Mantine UI

Desktop:
- Tauri 2.x

Database:
- SQLite

Terminal:
- xterm.js

SSH Backend:
- Windows OpenSSH (ssh.exe)

RDP Backend:
- mstsc.exe

Secret Storage:
- Windows DPAPI

State:
- Zustand

Build:
- MSI Installer

---

# Functional Requirements

## Server Management

Create/Edit/Delete Server

Fields:

- Name
- Host/IP
- Port
- Protocol (SSH/RDP)
- Username
- Group
- Tags
- Notes

Validation:
- Host required
- Name required
- Protocol required

---

## Group Management

Support tree structure.

Example:

Production
    Linux
    Windows
Development
DR

User can:
- Create group
- Rename group
- Delete group
- Move server between groups

---

## Search

Global search box.

Support:

gitlab
10.10.10
tag:k8s
group:production

Search updates instantly.

---

## Favorites

Server can be starred.

Favorites section displayed on top.

---

## SSH

Use xterm.js.

Backend launches:

ssh.exe

Requirements:

- Multiple tabs
- Copy/Paste
- UTF-8
- Resize support
- Reconnect button

No PuTTY.

No embedded legacy terminal.

---

## RDP

Launch:

mstsc.exe

Support:

- Fullscreen
- Username
- Custom .rdp profile

Double-click server launches RDP.

---

## Credentials

Create Credential Profiles.

Example:

Linux Root
Linux Admin
Windows Admin

Store secrets using Windows DPAPI.

Never store plaintext passwords.

---

## Import/Export

Import CSV:

name,host,protocol,username

Export CSV.

Export JSON backup.

Import JSON backup.

---

## Context Menu

Right click server:

- Connect
- Edit
- Delete
- Ping
- Copy IP
- Open RDP
- Open SSH

---

## Ping Tool

Display:

- Reachable
- Latency

Using:

ping.exe

---

## Settings

Theme:
- Light
- Dark

Default terminal font size.

Default SSH port.

Default RDP options.

---

# Non Functional Requirements

Startup:
< 2 seconds

Memory:
< 150MB idle

Database:
Local SQLite only

No cloud dependency.

No telemetry.

No login.

No subscription.

No internet requirement.

---

# Project Structure

src/

components/
pages/
store/
services/
hooks/
types/

src-tauri/

commands/
database/
security/
rdp/
ssh/

---

# Deliverables

Generate:

1. Complete source code
2. SQLite schema
3. Tauri commands
4. React UI
5. Build instructions
6. MSI packaging
7. README
8. Sample data

The application must compile successfully using:

npm install
npm run tauri build

Do not leave TODO placeholders.

Generate production-ready code.