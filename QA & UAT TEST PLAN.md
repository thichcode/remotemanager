# QA & UAT TEST PLAN

Act as:

- Senior QA Engineer
- Windows Desktop Tester
- Security Auditor
- Performance Engineer
- DevOps Release Engineer

Review the entire project and perform a complete validation.

---

# Build Validation

Verify:

- npm install
- npm run build
- npm run tauri build

Requirements:

- No TypeScript errors
- No ESLint errors
- No Rust compile errors
- No missing imports
- No broken dependencies

Fix automatically.

---

# Functional Testing

Test all features.

## Server CRUD

Create server

Edit server

Delete server

Duplicate server

Move server between groups

Expected:
No crashes.
Database updated correctly.

---

## Group CRUD

Create group

Rename group

Delete group

Nested groups

Move servers

Expected:
Tree refreshes correctly.

---

## Search

Test:

gitlab
10.10.10
prod
linux

Expected:
Realtime filtering.

No UI freeze.

---

## Favorites

Add favorite

Remove favorite

Restart application

Expected:
Favorites persist.

---

## SSH

Test:

Valid host

Invalid host

Connection timeout

Host unreachable

Wrong username

Wrong key

Resize terminal

Copy/Paste

UTF8 characters

Long output

Expected:
No terminal corruption.

No memory leak.

No application crash.

---

## RDP

Test:

Valid server

Invalid IP

DNS name

Fullscreen mode

Custom port

Expected:
mstsc launches correctly.

Error messages displayed.

---

## Credential Profiles

Create profile

Edit profile

Delete profile

Use profile

Restart app

Expected:
Credentials preserved.

Passwords never stored plaintext.

---

## Import CSV

Valid CSV

Invalid CSV

Empty CSV

Large CSV (1000 servers)

Expected:
Validation errors handled.

No crashes.

---

## Export CSV

Verify output format.

Re-import exported file.

Expected:
Data integrity maintained.

---

## JSON Backup

Export backup

Delete database

Restore backup

Expected:
100% recovery.

---

# Security Review

Search codebase for:

password
secret
token
apikey

Verify:

No plaintext storage.

No hardcoded credentials.

DPAPI used correctly.

No secrets logged.

No credentials written to crash logs.

---

# SQL Injection Testing

Attempt:

'
"
--
OR 1=1

in all input fields.

Expected:

No SQL injection.

Use prepared statements only.

---

# Path Traversal Testing

Attempt:

../../
..\\..
C:\\Windows

Expected:

Blocked.

---

# Crash Testing

Randomly create:

100 groups

1000 servers

50 tabs

Repeated imports

Repeated deletes

Expected:

Application remains stable.

---

# Memory Testing

Measure:

Startup RAM

10 SSH tabs

50 SSH tabs

1000 servers loaded

Expected:

Memory usage reasonable.

No leak detected.

---

# Performance Testing

Measure:

Startup time

Search latency

Database query latency

Import 1000 records

Export 1000 records

Expected:

Startup < 2 seconds

Search < 100ms

---

# UI Testing

Dark Mode

High DPI

4K monitor

125%

150%

200%

Expected:

No layout break.

---

# Release Readiness

Generate:

- BUG REPORT
- SECURITY REPORT
- PERFORMANCE REPORT
- BUILD REPORT

Fix all critical issues.

Repeat testing until:

Critical = 0
High = 0

Return final release candidate.