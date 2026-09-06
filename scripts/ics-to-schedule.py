#!/usr/bin/env python3
"""通用 ICS → Loop schedule.json 转换器。

面向任意日历应用导出的课表 ICS（WakeUp、超星、系统日历等），在主机上
生成设备可读的 schedule.json。设备端不解析 ICS。

支持（RFC 5545 常用子集）：
  - VEVENT + SUMMARY / LOCATION / DESCRIPTION / DTSTART / DTEND / DURATION
  - RRULE: FREQ=WEEKLY, INTERVAL, UNTIL, COUNT, BYDAY
  - EXDATE（按日排除）
  - 行折叠、嵌套 VALARM/忽略、UTF-8 BOM
  - TZID / UTC → 本地墙钟（默认 Asia/Shanghai = UTC+8，无夏令时）

输出 schema 见 docs/SCHEDULE.md。
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from dataclasses import dataclass, field
from datetime import date, datetime, timedelta, timezone
from pathlib import Path

PERIOD_PATTERNS = (
    re.compile(r"第\s*(\d+)\s*[-~—－到至]\s*(\d+)\s*节"),
    re.compile(r"第\s*(\d+)\s*节"),
    re.compile(r"(\d+)\s*[-~—－]\s*(\d+)\s*节"),
)

BYDAY_MAP = {
    "MO": 1,
    "TU": 2,
    "WE": 3,
    "TH": 4,
    "FR": 5,
    "SA": 6,
    "SU": 7,
}

# 固定偏移；若需其它时区用 --utc-offset-hours。
DEFAULT_UTC_OFFSET_HOURS = 8


@dataclass
class Event:
    summary: str
    location: str
    description: str
    dtstart: datetime  # naive local wall clock
    dtend: datetime
    until: date | None = None
    count: int | None = None
    interval: int = 1
    byday: list[int] = field(default_factory=list)
    exdates: set[date] = field(default_factory=set)
    uid: str = ""


def unfold(text: str) -> str:
    text = text.replace("\r\n", "\n").replace("\r", "\n")
    return re.sub(r"\n[ \t]", "", text)


def unescape(value: str) -> str:
    return (
        value.replace("\\n", "\n")
        .replace("\\N", "\n")
        .replace("\\,", ",")
        .replace("\\;", ";")
        .replace("\\\\", "\\")
    )


def strip_nested_components(body: str) -> str:
    """去掉 VEVENT 内嵌套组件（VALARM 等），避免字段被覆盖。"""
    out: list[str] = []
    depth = 0
    for line in body.split("\n"):
        if line.startswith("BEGIN:"):
            depth += 1
            continue
        if line.startswith("END:"):
            depth = max(0, depth - 1)
            continue
        if depth == 0:
            out.append(line)
    return "\n".join(out)


def parse_prop(line: str) -> tuple[str, dict[str, str], str] | None:
    if ":" not in line:
        return None
    meta, value = line.split(":", 1)
    parts = meta.split(";")
    name = parts[0].upper()
    params: dict[str, str] = {}
    for part in parts[1:]:
        if "=" in part:
            k, v = part.split("=", 1)
            params[k.upper()] = v
    return name, params, value


def to_local(dt: datetime, utc_offset_hours: int) -> datetime:
    if dt.tzinfo is not None:
        local = timezone(timedelta(hours=utc_offset_hours))
        return dt.astimezone(local).replace(tzinfo=None)
    return dt


def parse_dt(value: str, params: dict[str, str], utc_offset_hours: int) -> datetime:
    value = value.strip()
    if params.get("VALUE", "").upper() == "DATE" or (
        "T" not in value and len(value) >= 8 and value[:8].isdigit()
    ):
        d = datetime.strptime(value[:8], "%Y%m%d")
        return d
    if value.endswith("Z"):
        dt = datetime.strptime(value, "%Y%m%dT%H%M%SZ").replace(tzinfo=timezone.utc)
        return to_local(dt, utc_offset_hours)
    # TZID 参数：多数课表导出已是本地墙钟，按 naive 解析。
    return datetime.strptime(value[:15], "%Y%m%dT%H%M%S")


def parse_duration(value: str) -> timedelta:
    # 简化：PT1H30M / PT90M / P1DT2H
    match = re.fullmatch(
        r"P(?:(\d+)D)?(?:T(?:(\d+)H)?(?:(\d+)M)?(?:(\d+)S)?)?",
        value.upper(),
    )
    if not match:
        raise ValueError(f"unsupported DURATION: {value}")
    days, hours, minutes, seconds = (int(x or 0) for x in match.groups())
    return timedelta(days=days, hours=hours, minutes=minutes, seconds=seconds)


def parse_rrule(value: str) -> dict[str, str]:
    parts: dict[str, str] = {}
    for item in value.split(";"):
        if "=" not in item:
            continue
        k, v = item.split("=", 1)
        parts[k.upper()] = v
    return parts


def parse_ics(text: str, utc_offset_hours: int) -> list[Event]:
    if text.startswith("\ufeff"):
        text = text[1:]
    text = unfold(text)
    events: list[Event] = []
    for block in text.split("BEGIN:VEVENT")[1:]:
        body = block.split("END:VEVENT", 1)[0]
        body = strip_nested_components(body)
        fields: dict[str, tuple[dict[str, str], str]] = {}
        exdates: set[date] = set()
        for line in body.split("\n"):
            if not line:
                continue
            parsed = parse_prop(line)
            if parsed is None:
                continue
            name, params, value = parsed
            value = unescape(value).strip()
            if name == "EXDATE":
                # 可含逗号分隔多个日期
                for piece in value.split(","):
                    piece = piece.strip()
                    if not piece:
                        continue
                    exdates.add(parse_dt(piece, params, utc_offset_hours).date())
                continue
            # 同名属性保留第一次（避免重复行覆盖）。
            if name not in fields:
                fields[name] = (params, value)

        if "SUMMARY" not in fields or "DTSTART" not in fields:
            continue
        start_params, start_raw = fields["DTSTART"]
        dtstart = parse_dt(start_raw, start_params, utc_offset_hours)
        # 全天事件：无时刻，课表场景跳过。
        if "T" not in start_raw and start_params.get("VALUE", "").upper() == "DATE":
            continue
        if "T" not in start_raw and len(start_raw) == 8:
            continue

        if "DTEND" in fields:
            end_params, end_raw = fields["DTEND"]
            dtend = parse_dt(end_raw, end_params, utc_offset_hours)
        elif "DURATION" in fields:
            dtend = dtstart + parse_duration(fields["DURATION"][1])
        else:
            dtend = dtstart + timedelta(minutes=45)

        if dtend <= dtstart:
            continue

        until: date | None = None
        count: int | None = None
        interval = 1
        byday: list[int] = []
        if "RRULE" in fields:
            rule = parse_rrule(fields["RRULE"][1])
            freq = rule.get("FREQ", "WEEKLY").upper()
            if freq != "WEEKLY":
                # 非周循环：当作单次事件（只用 DTSTART 当天）。
                interval = 1
                until = dtstart.date()
                count = 1
            else:
                interval = max(1, int(rule.get("INTERVAL", "1")))
                if "UNTIL" in rule:
                    until_dt = parse_dt(rule["UNTIL"], {}, utc_offset_hours)
                    until = until_dt.date()
                if "COUNT" in rule:
                    count = max(1, int(rule["COUNT"]))
                if "BYDAY" in rule:
                    for token in rule["BYDAY"].split(","):
                        token = re.sub(r"^[+-]?\d+", "", token.strip().upper())
                        if token in BYDAY_MAP:
                            byday.append(BYDAY_MAP[token])

        location = re.sub(r"\s+", " ", fields.get("LOCATION", ({}, ""))[1]).strip()
        description = fields.get("DESCRIPTION", ({}, ""))[1]
        events.append(
            Event(
                summary=fields["SUMMARY"][1],
                location=location,
                description=description,
                dtstart=dtstart,
                dtend=dtend,
                until=until,
                count=count,
                interval=interval,
                byday=byday,
                exdates=exdates,
                uid=fields.get("UID", ({}, ""))[1],
            )
        )
    return events


def iso_weekday(dt: datetime) -> int:
    return dt.weekday() + 1


def minutes_of_day(dt: datetime) -> int:
    return dt.hour * 60 + dt.minute


def period_label(*texts: str) -> str:
    for text in texts:
        for line in text.splitlines():
            line = line.strip()
            if not line:
                continue
            for pattern in PERIOD_PATTERNS:
                match = pattern.search(line)
                if not match:
                    continue
                groups = match.groups()
                if len(groups) == 1:
                    return f"第{groups[0]}节"
                return f"第{groups[0]}-{groups[1]}节"
    return ""


def week_index(term_start: date, day: date) -> int:
    if day < term_start:
        return 0
    return ((day - term_start).days // 7) + 1


def occurrence_weeks(event: Event, term_start: date, weekday: int) -> list[int]:
    """根据 RRULE 展开教学周列表（相对 term_start）。

    ``weekday`` 用于 EXDATE 过滤（1=周一 … 7=周日）。
    """
    start_week = week_index(term_start, event.dtstart.date())
    if start_week <= 0:
        # 学期开始前的首堂课：仍从第 1 周起算该 weekday 的第一次。
        start_week = 1

    weeks: list[int] = []
    if event.count is not None:
        for i in range(event.count):
            weeks.append(start_week + i * event.interval)
    elif event.until is not None:
        end_week = week_index(term_start, event.until)
        if end_week <= 0:
            end_week = start_week
        w = start_week
        while w <= end_week:
            weeks.append(w)
            w += event.interval
    else:
        # 无 UNTIL/COUNT：仅首周一次。
        weeks.append(start_week)

    # EXDATE：去掉该 weekday 对应教学周。
    if event.exdates:
        filtered: list[int] = []
        term_wd = term_start.weekday() + 1  # 1=Mon
        delta = (weekday - term_wd) % 7
        for w in weeks:
            day = term_start + timedelta(days=(w - 1) * 7 + delta)
            if day not in event.exdates:
                filtered.append(w)
        weeks = filtered

    return [w for w in weeks if w >= 1]


def compress_weeks(weeks: list[int], interval: int) -> list[tuple[int, int, int]]:
    """把周次压成 (weeks_start, weeks_end, week_interval) 段。"""
    if not weeks:
        return []
    weeks = sorted(set(weeks))
    if interval <= 1:
        # 连续段合并
        ranges: list[tuple[int, int, int]] = []
        start = prev = weeks[0]
        for w in weeks[1:]:
            if w == prev + 1:
                prev = w
                continue
            ranges.append((start, prev, 1))
            start = prev = w
        ranges.append((start, prev, 1))
        return ranges

    # 固定间隔：整段用同一 interval 覆盖 min..max（中间缺周由 interval 表达）。
    # 若序列不符合等差，拆成多段。
    ranges = []
    start = prev = weeks[0]
    for w in weeks[1:]:
        if w == prev + interval:
            prev = w
            continue
        ranges.append((start, prev, interval))
        start = prev = w
    ranges.append((start, prev, interval))
    return ranges


def event_weekdays(event: Event) -> list[int]:
    """展开 BYDAY；无 BYDAY 时用 DTSTART 的星期几。"""
    if event.byday:
        seen: set[int] = set()
        out: list[int] = []
        for day in event.byday:
            if day not in seen:
                seen.add(day)
                out.append(day)
        return out
    return [iso_weekday(event.dtstart)]


def monday_of_week(day: date) -> date:
    """返回 day 所在 ISO 周（周一…周日）的周一。"""
    return day - timedelta(days=day.weekday())  # Monday=0


def convert(
    events: list[Event],
    term_name: str,
    term_start: date | None,
) -> dict:
    if not events:
        raise SystemExit("ICS 中没有可用的 VEVENT")
    if term_start is None:
        # 第 1 教学周从最早上课日所在周的周一起算。
        term_start = monday_of_week(min(e.dtstart.date() for e in events))

    courses: list[dict] = []
    next_id = 1
    skipped = 0
    for event in events:
        label = period_label(event.description, event.summary)
        emitted = False
        for weekday in event_weekdays(event):
            weeks = occurrence_weeks(event, term_start, weekday)
            if not weeks:
                continue
            emitted = True
            for weeks_start, weeks_end, interval in compress_weeks(weeks, event.interval):
                course = {
                    "id": next_id,
                    "name": event.summary,
                    "location": event.location,
                    "weekday": weekday,
                    "start_min": minutes_of_day(event.dtstart),
                    "end_min": minutes_of_day(event.dtend),
                    "period_label": label,
                    "weeks_start": weeks_start,
                    "weeks_end": weeks_end,
                }
                if interval != 1:
                    course["week_interval"] = interval
                courses.append(course)
                next_id += 1
        if not emitted:
            skipped += 1

    if not courses:
        raise SystemExit("转换结果为空（可能全被 EXDATE / 学期过滤）")

    payload = {
        "version": 1,
        "term": {
            "name": term_name,
            "start_date": term_start.isoformat(),
        },
        "courses": courses,
    }
    if skipped:
        print(f"warning: skipped {skipped} events with no weeks", file=sys.stderr)
    return payload


def parse_ymd(value: str) -> date:
    return date.fromisoformat(value)


def main() -> int:
    parser = argparse.ArgumentParser(
        description="将课表 ICS 转为 Loop schedule.json（通用）",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""示例:
  python scripts/ics-to-schedule.py 课表.ics
  python scripts/ics-to-schedule.py 课表.ics -o schedule.json --term-name 2026秋
  python scripts/ics-to-schedule.py 课表.ics --term-start 2026-09-01 --stdout
""",
    )
    parser.add_argument("ics", type=Path, help="输入 .ics 文件")
    parser.add_argument(
        "-o",
        "--output",
        type=Path,
        default=None,
        help="输出 JSON 路径（默认 ./schedule.json）",
    )
    parser.add_argument("--term-name", default=None, help="学期名称（默认按起始年推断）")
    parser.add_argument(
        "--term-start",
        type=parse_ymd,
        default=None,
        help="学期第 1 周起始日 YYYY-MM-DD（默认取最早上课日所在周的周一）",
    )
    parser.add_argument(
        "--utc-offset-hours",
        type=int,
        default=DEFAULT_UTC_OFFSET_HOURS,
        help="UTC 事件转到本地的固定偏移小时（默认 8 = 上海）",
    )
    parser.add_argument("--stdout", action="store_true", help="打印到标准输出而不写文件")
    parser.add_argument(
        "--check",
        action="store_true",
        help="只校验/预览，打印课程数与周次摘要",
    )
    args = parser.parse_args()

    text = args.ics.read_text(encoding="utf-8")
    events = parse_ics(text, args.utc_offset_hours)
    term_start = args.term_start
    if term_start is None and events:
        term_start = monday_of_week(min(e.dtstart.date() for e in events))
    term_name = args.term_name
    if term_name is None and term_start is not None:
        # 8–12 月视为秋，否则春
        term_name = f"{term_start.year}{'秋' if term_start.month >= 8 else '春'}"
    elif term_name is None:
        term_name = "学期"

    payload = convert(events, term_name, term_start)

    if args.check:
        print(f"events={len(events)} courses={len(payload['courses'])}")
        print(f"term={payload['term']['name']} start={payload['term']['start_date']}")
        for course in payload["courses"][:8]:
            interval = course.get("week_interval", 1)
            print(
                f"  #{course['id']} wd={course['weekday']} "
                f"{course['start_min']}-{course['end_min']} "
                f"w{course['weeks_start']}-{course['weeks_end']}/i{interval} "
                f"{course['name']}"
            )
        if len(payload["courses"]) > 8:
            print(f"  ... {len(payload['courses']) - 8} more")
        return 0

    text_out = json.dumps(payload, ensure_ascii=False, indent=2) + "\n"
    if args.stdout:
        sys.stdout.write(text_out)
        return 0

    output = args.output or Path("schedule.json")
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(text_out, encoding="utf-8")
    print(f"wrote {len(payload['courses'])} courses -> {output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
