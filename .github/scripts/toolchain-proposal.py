"""Pure observation-to-decision policy; no Git, subprocess, or network access."""
import json
import re
import sys


class Refused(ValueError):
    pass


def require(condition, reason):
    if not condition:
        raise Refused(reason)


def version(value):
    # A dated nightly, which is what this repository pins. ISO dates order
    # lexically, so the string itself is the comparison -- no calendar parsing,
    # and a date rustup does not publish fails at install, which is a better
    # place to find out than a regex encoding month lengths.
    require(isinstance(value, str) and re.fullmatch(r"nightly-\d{4}-\d{2}-\d{2}", value),
            "unsupported channel: expected a dated nightly")
    return value


def sha(value):
    require(isinstance(value, str) and re.fullmatch(r"[0-9a-f]{40}", value), "unsupported object identity")
    return value


def regular_entry(value):
    require(isinstance(value, str) and re.fullmatch(
        r"100644 blob [0-9a-f]{40}\trust-toolchain\.toml\n", value), "unsupported pin path or mode")


def decide(data):
    phase = data["phase"]
    require(phase in ("observe", "prepared"), "unsupported phase")
    repo = data["repo"]
    require(isinstance(repo, str) and re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repo),
            "unsupported repository identity")
    latest, channel = data["latest"], data["channel"]
    require(version(latest) > version(channel), "no forward channel proposal")
    branch = "chore/toolchain-" + latest
    base = data["base"]
    require(base["ref"] == "refs/heads/dev" and base["object"]["type"] == "commit", "wrong base ref")
    base_sha = sha(base["object"]["sha"])
    require(data["checkout"] == base_sha and data["dirty"] == "", "checkout is not clean current dev")
    regular_entry(data["base_entry"])
    before = bytes.fromhex(data["before_hex"])
    before.decode("utf-8", errors="strict")
    old = ('channel = "' + channel + '"').encode()
    channel_lines = [line for line in before.splitlines() if re.match(rb"\s*channel\s*=", line)]
    require(channel_lines == [old], "unsupported or duplicate channel declaration")
    require(before.count(old + b"\n") == 1, "pin requires one complete canonical channel line")
    expected = before.replace(old + b"\n", ('channel = "' + latest + '"\n').encode(), 1)
    pages = data["pages"]
    require(isinstance(pages, list) and len(pages) > 0 and all(isinstance(page, list) for page in pages),
            "incomplete PR census")
    proposals = [proposal for page in pages for proposal in page]
    require(len(proposals) <= 1, "ambiguous PR census")
    status, remote = data["remote_status"], data["remote_ref"]
    require(type(status) is int and status in (0, 2), "remote branch lookup failed")
    if status == 2:
        require(remote == "", "inconsistent absent ref")
        require(not proposals, "PR exists without the expected branch")
        if phase == "observe":
            require(all(data[key] == "" for key in ("fetched", "parents", "changes", "head_entry", "after_hex")),
                    "unexpected objects for absent branch")
            return {"action": "new", "probed": False}
        head = sha(data["fetched"])
    else:
        require(phase == "observe", "remote branch appeared before push")
        require(isinstance(remote, str) and remote == data["fetched"] + "\trefs/heads/" + branch + "\n",
                "remote and fetched branch identities differ")
        head = sha(data["fetched"])
    require(data["parents"] == head + " " + base_sha, "branch is not one ordinary commit on current dev")
    require(data["changes"] == "M\trust-toolchain.toml\n", "branch changes exceed the single pin")
    regular_entry(data["head_entry"])
    require(bytes.fromhex(data["after_hex"]) == expected, "branch bytes are not the exact channel edit")
    if not proposals:
        return {"action": "push" if phase == "prepared" else "recover", "head_sha": head, "probed": False}
    proposal = proposals[0]
    require(proposal["state"] == "open" and proposal["merged_at"] is None, "proposal is closed or unsupported")
    require(type(proposal["number"]) is int and proposal["number"] > 0, "missing proposal identity")
    require(proposal["head"]["repo"]["full_name"] == repo and proposal["base"]["repo"]["full_name"] == repo,
            "proposal repository mismatch")
    require(proposal["head"]["ref"] == branch and proposal["base"]["ref"] == "dev"
            and proposal["head"]["sha"] == head, "proposal ref or object mismatch")
    return {"action": "open", "number": proposal["number"], "head_sha": head, "probed": False}


if __name__ == "__main__":
    try:
        raw = sys.stdin.buffer.read(2_000_001)
        require(len(raw) <= 2_000_000, "observation exceeds supported size")
        print(json.dumps(decide(json.loads(raw))))
    except Refused as error:
        print("proposal refused: " + str(error), file=sys.stderr)
        sys.exit(1)
    except (KeyError, TypeError, ValueError, OverflowError):
        print("proposal refused: malformed observation", file=sys.stderr)
        sys.exit(1)
