#!/usr/bin/env python3
"""将 WakeUp 导出的课表 ICS 转为 Loop 内置 schedule.json。

设备端不解析 ICS。本脚本在主机上一次性生成 fixtures/schedule.json，
再由 loop-core 通过 include_str! 编进固件。
"""

from __future__ import annotations

import argparse
import json
import re
from dataclasses import dataclass
from datetime import date, datetime, timedelta, timezone
from pathlib import Path

PERIOD_RE = re.compile(r"第\s*(\d+)\s*-\s*(\d+)\s*节")


@dataclass
class Event:
    summary: str
    location: str
    description: str
    dtstart: datetime
    dtend: datetime
    until: datetime | None


def unfold(text: str) -> str:
    return text.replace("\r\n ", "").replace("\n ", "").replace("\r\n", "\n")


def parse_ics(text: str) -> list[Event]:
    text = unfold(text)
    events: list[Event] = []
    blocks = text.split("BEGIN:VEVENT")
    for block in blocks[1:]:
        body = block.split("END:VEVENT", 1)[0]
        fields: dict[str, str] = {}
        for line in body.split("\n"):
            if not line or line.startswith("BEGIN:") or line.startswith("END:"):
                continue
            if ":" not in line:
                continue
            key, value = line.split(":", 1)
            key = key.split(";", 1)[0]
            # VALARM 内 DESCRIPTION 覆盖外层时，保留第一条课程 DESCRIPTION。
            if key in fields and key == "DESCRIPTION":
                continue
            fields[key] = value.replace("\\n", "\n").strip()
        if "SUMMARY" not in fields or "DTSTART" not in fields or "DTEND" not in fields:
            continue
        dtstart = parse_dt(fields["DTSTART"])
        dtend = parse_dt(fields["DTEND"])
        until = None
        if "RRULE" in fields:
            for part in fields["RRULE"].split(";"):
                if part.startswith("UNTIL="):
                    until = parse_dt(part.split("=", 1)[1])
        events.append(
            Event(
                summary=fields["SUMMARY"],
                location=re.sub(r"\s+", " ", fields.get("LOCATION", "")).strip(),
                description=fields.get("DESCRIPTION", ""),
                dtstart=dtstart,
                dtend=dtend,
                until=until,
            )
        )
    return events


def parse_dt(value: str) -> datetime:
    # DTSTART;TZID=Asia/Shanghai:20260901T075000 或 UNTIL=20261221T160000Z
    if "T" not in value:
        return datetime.strptime(value[:8], "%Y%m%d")
    raw = value
    if raw.endswith("Z"):
        return datetime.strptime(raw, "%Y%m%dT%H%M%SZ").replace(tzinfo=timezone.utc)
    # 本地墙钟，按 naive 处理（Asia/Shanghai，无夏令时）。
    return datetime.strptime(raw[:15], "%Y%m%dT%H%M%S")


def iso_weekday(dt: datetime) -> int:
    # Python: Monday=0 → 我们要 Monday=1。
    return dt.weekday() + 1


def minutes_of_day(dt: datetime) -> int:
    return dt.hour * 60 + dt.minute


def period_label(description: str) -> str:
    first = description.split("\n", 1)[0]
    match = PERIOD_RE.search(first)
    if not match:
        return first.strip()
    return f"第{match.group(1)}-{match.group(2)}节"


def week_index(term_start: date, day: date) -> int:
    if day < term_start:
        return 0
    return ((day - term_start).days // 7) + 1


def convert(events: list[Event], term_name: str) -> dict:
    if not events:
        raise SystemExit("ICS 中没有 VEVENT")
    term_start = min(e.dtstart.date() for e in events)
    courses = []
    for index, event in enumerate(events, start=1):
        until_day = event.until.date() if event.until else event.dtend.date()
        # UNTIL 常为 UTC 当天末；用事件本地星期对齐到最后一堂课日期。
        # 简化：以 until 日期所在教学周为 weeks_end。
        weeks_start = week_index(term_start, event.dtstart.date())
        weeks_end = max(weeks_start, week_index(term_start, until_day))
        courses.append(
            {
                "id": index,
                "name": event.summary,
                "location": event.location,
                "weekday": iso_weekday(event.dtstart),
                "start_min": minutes_of_day(event.dtstart),
                "end_min": minutes_of_day(event.dtend),
                "period_label": period_label(event.description),
                "weeks_start": weeks_start,
                "weeks_end": weeks_end,
            }
        )
    return {
        "version": 1,
        "term": {
            "name": term_name,
            "start_date": term_start.isoformat(),
        },
        "courses": courses,
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("ics", type=Path, help="WakeUp 导出的 .ics")
    parser.add_argument(
        "-o",
        "--output",
        type=Path,
        default=Path(__file__).resolve().parents[1] / "fixtures" / "schedule.json",
    )
    parser.add_argument("--term-name", default="2026秋")
    args = parser.parse_args()
    text = args.ics.read_text(encoding="utf-8")
    payload = convert(parse_ics(text), args.term_name)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(
        json.dumps(payload, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    print(f"wrote {len(payload['courses'])} courses -> {args.output}")


if __name__ == "__main__":
    main()
