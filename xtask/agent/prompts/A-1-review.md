# Task: classify one recorded exception

A team declared an exception (an `ovr` directive in a code comment) to one of its rules. The
attached context shows the code around it, what the checking tool reports for that file today, the
project's sealed history (ledger lines, which record seals by version) and the date of this review.

Classify the exception as exactly one of:

- **expired**: its expiry (`exp=` a date, or `until=` a version) has passed by the review date or
  by the latest sealed version, or the tool reports it as expired.
- **stale**: still in date, but no longer needed: the code it excuses no longer does what the rule
  forbids, the excused code was removed or moved away, the reason it gives no longer holds, or it
  names a rule or line that does not match what is there.
- **live**: in date and still needed: the excused code is present, still does what the rule
  forbids, and the reason still holds.

Judge only from the attached context. Do not run commands or open other files.

Answer with two or three sentences of reasoning, then one final line exactly in this form:

LABEL: stale

(or `LABEL: expired`, or `LABEL: live`).
