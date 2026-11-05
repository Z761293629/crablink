# Codes are never reused

Deleting a Link keeps its row as a Deleted Link instead of removing it, so its Code can never be taken again; visiting a deleted or expired Link returns 410 Gone. Short URLs end up on posters, QR codes and old messages we can't recall, so letting a Code point somewhere new would silently send those readers to an unrelated (possibly malicious, once the service is public) Target.
