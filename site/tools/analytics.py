"""Put the visit counter into every page of the site, at deploy time.

One place rather than twenty-five: a guide added tomorrow is counted without
anybody remembering to paste a snippet into it. Run by the site workflow on the
built site, before it is uploaded:

    python3 site/tools/analytics.py site

What is counted, and what is not, is written on /telemetry/. The website's
counter is PostHog, in its EU region. It keeps no cookie and nothing that
outlives the tab: the visitor id lives in session storage, so one visit is
followed from page to page and forgotten when the tab closes. Do Not Track and
Global Privacy Control load nothing at all, no location is looked up, and
session replay masks everything typed.
"""

import pathlib
import sys

KEY = "phc_Ck3ThjdcBuapV26zSbxsNiZBbqg52pthagE4v66hbadA"
HOST = "https://eu.i.posthog.com"

# PostHog's own loader: a stub that queues calls until the library arrives.
LOADER = (
    '!function(t,e){var o,n,p,r;e.__SV||(window.posthog=e,e._i=[],e.init=function(i,s,a){'
    'function g(t,e){var o=e.split(".");2==o.length&&(t=t[o[0]],e=o[1]),t[e]=function(){'
    't.push([e].concat(Array.prototype.slice.call(arguments,0)))}}(p=t.createElement("script"))'
    '.type="text/javascript",p.crossOrigin="anonymous",p.async=!0,p.src=s.api_host.replace('
    '".i.posthog.com","-assets.i.posthog.com")+"/static/array.js",(r=t.getElementsByTagName('
    '"script")[0]).parentNode.insertBefore(p,r);var u=e;for(void 0!==a?u=e[a]=[]:a="posthog",'
    'u.people=u.people||[],u.toString=function(t){var e="posthog";return"posthog"!==a&&(e+="."+a),'
    't||(e+=" (stub)"),e},u.people.toString=function(){return u.toString(1)+".people (stub)"},'
    'o="init capture register register_once register_for_session unregister unregister_for_session '
    'getFeatureFlag getFeatureFlagPayload isFeatureEnabled reloadFeatureFlags identify '
    'setPersonProperties group resetGroups reset get_distinct_id get_session_id '
    'get_session_replay_url alias set_config startSessionRecording stopSessionRecording '
    'sessionRecordingStarted captureException get_property getSessionProperty opt_in_capturing '
    'opt_out_capturing has_opted_in_capturing has_opted_out_capturing debug".split(" "),'
    'n=0;n<o.length;n++)g(u,o[n]);e._i.push([i,s,a])},e.__SV=1)}(document,window.posthog||[]);'
)

CONFIG = (
    "posthog.init('%s',{api_host:'%s',persistence:'sessionStorage',"
    "person_profiles:'identified_only',respect_dnt:true,capture_pageview:true,"
    "capture_pageleave:true,session_recording:{maskAllInputs:true},"
    # No location is looked up for a visit, as none is for the app's counts.
    # And every event says it came from the website: the site and the app
    # share one PostHog project, and `source` is the filter that keeps them
    # apart (the app's single event is `deck usage`, sent by `$lib` deck).
    "loaded:function(p){p.register({$geoip_disable:true,source:'website'})}});" % (KEY, HOST)
)

# A visitor who asked not to be tracked, by Do Not Track or Global Privacy
# Control, gets nothing: not the counter, and not the request that fetches it.
SNIPPET = (
    "<script>if(navigator.doNotTrack!=='1'&&window.doNotTrack!=='1'&&!navigator.globalPrivacyControl){"
    + LOADER
    + CONFIG
    + "}</script>\n"
)


def main(root: str) -> None:
    pages = sorted(pathlib.Path(root).rglob("*.html"))
    counted = already = 0
    for page in pages:
        html = page.read_text(encoding="utf-8")
        if "posthog.init(" in html:
            already += 1
            continue
        at = html.lower().find("</head>")
        if at < 0:
            # A fragment, not a page: shown inside another page, which
            # already counts the visit.
            continue
        page.write_text(html[:at] + SNIPPET + html[at:], encoding="utf-8")
        counted += 1
    if counted + already == 0:
        sys.exit("no page was given the counter")
    print(f"counted {counted} of {len(pages)} pages")


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else "site")
