# Air Reinforcement Charts (§34.84 Commonwealth / §34.85 Axis)

Transcription of the two Airplane Reinforcement charts — the per-month schedule of **planes** each
side receives into its pool over the campaign (Sept 1940 → Dec 1942, GT2–GT110). Planes are
assigned to squadron counters (SGSUs) by the player (§35.2); the charts do **not** list squadrons or
bases (those come from the §60 initial air setup + the SGSU counter mix).

**Sources:** §34.84 Commonwealth = PDF p143; §34.85 Axis = rulebook screenshot (2026-09-10).
**Rule note (both charts):** each entry is the number of that plane type received *during that
month*, **divided as evenly as possible across the month's four Game-Turns** unless a specific GT is
given. `WD` = squadron withdrawals (remove that many squadrons of the capability class at the start
of the indicated month): **F** = a ≥12-plane CAP-capable fighter squadron, **B** = a ≥12-plane
bombing-capable squadron, **R** = a ≥12-plane recon-capable squadron. `*`/`**`/`†`/`‡` footnotes per
the charts (torpedo-bomber SM79 variants; multi-squadron / bomb-load withdrawal qualifiers).

> **Status: first-pass transcription from images — verify before loading.** Dense numeric tables;
> a few glyphs are ambiguous and flagged `(?)`. This is the data record; converting it to a loadable
> RON schedule + a plane-pool reinforcement mechanism is the follow-on engine task.

## Aircraft name → engine key (`data/reference/aircraft/*.ron`)

**Commonwealth:** Fulmar→`Fulmar Mk.II` · Hurricane I→`Hurricane Mk.I` · Hurricane IIA/IIB/IIC/IID→
`Hurricane Mk.IIA/IIB/IIC/IID` · Blenheim I→`Blenheim Mk.I` · Blenheim IV→`Blenheim Mk.IV` ·
Blenheim IVF→`Blenheim Mk.IVF` · Skua II→`Skua Mk.II` · Wellington I/IV→`Wellington Mk.I/IV` ·
Maryland→`167 Maryland` · Tomahawk→`P-40 Tomahawk` · Kittyhawk I/II/III→`P-40 Kittyhawk I/II/III` ·
Lysander (I)→`Lysander Mk.I` · Beaufighter IF→`Beaufighter Mk.IF` · Beaufighter VI→`Beaufighter Mk.VIF` ·
Gladiator II→`Gladiator Mk.II` · Wildcat→`F4F Wildcat` · Albacore→`Albacore` · Spitfire VB/VC→
`Spitfire Mk.VB/VC` · Beaufort I→`Beaufort Mk.I` · Boston III→`Boston Mk.III` · Hudson→`A-28 Hudson` ·
Baltimore (I)→`A-30 Baltimore I` · Caudron→`C.714` · B17D→`B-17/D Flying Fortress` ·
B24→`B-24/PB4y Liberator III` · B25→`B-25 Mitchell II` · B26→`B-26 Marauder` · Anson (I)→`Anson Mk.I` ·
Sunderland→`Sunderland` · Halifax II→`Halifax Mk.II` · Bombay→`Bombay Mk.I` · Tiger Moth II→
`Tiger Moth Mk.II` · Dewoitine→`D.520` · Potez→`63.11`.

**German (`Ge:`):** Bf109E/F/G→`Bf.109E/F/G` · Bf110→`Bf.110` · Ju52→`Ju.52/3m` · Ju87B/D→`Ju.87B/D` ·
Ju88C→`Ju.88c` · Ju88D→`Ju.88D` · He111→`He.111` · FW200C→`Fw.200 C` · Hs126→`Hs.126` · Arado196→`Ar.196`.

**Italian (`It:`):** CR32→`C.R.32` · CR42→`C.R.42 Falco` · CR42AS→`C.R.42AS Falco` · C200→`C.200 Saetta` ·
C202→`C.202 Folgore` · G50→`G.50 Freccia` · Re2000→`Re.2000 Falco I` · Re2001→`Re.2001 Falco II` ·
Ba65→`Ba 65` · BR20→`B.R.20 Cicogna` · Ca311→`Ca 311` · Z501→`Cant Z.501 Gabbiano` · Z506B→
`Cant Z.506B Airone/35` · Z1007(bis)→`Cant Z.1007bis Alcione` · Ro37bis→`Ro 37bis` ·
SM79→`S.M.79 Sparviero` · SM81→`S.M.81 Pipistrello` · SM84→`S.M.84`.

---

## §34.84 — Commonwealth (PDF p143)

- **1940 Sep IV (GT2):** 3× Fulmar, 2× Hurricane I
- **Oct I (GT3):** 30× Blenheim I, 6× Skua II. **WD:** 1F
- **Nov II (GT8):** 4× B17D. **WD:** 2F, 2B\*\*
- **Dec I (GT11):** 12× Blenheim IV, 40× Wellington I. **WD:** 1B
- **1941 Jan (GT15–18):** 44× Hurricane I, 12× Blenheim I, 1× Wellington I. **WD:** 1F, 2B\*\*
- **Feb (GT19–22):** 30× Hurricane I, 24× Blenheim I, 14× Blenheim IV, 2× Maryland, 2× Wellington I. **WD:** 1F
- **Mar (GT23–26):** 14× Blenheim IVF, 13× Hurricane I, 2× Tomahawk, 21× Blenheim I, 6× Maryland, 6× Lysander, 16× Wellington. **WD:** 1B
- **Apr (GT27–30):** 66× Hurricane I, 25× Tomahawk, 18× Blenheim I, 10× Blenheim IV, 6× Maryland, 6× Lysander I, 13× Wellington I, 14× Wellington IV. **WD:** 1F
- **May (GT31–34):** 16× Beaufighter IF, 65× Hurricane I, 21× Tomahawk, 20× Blenheim I, 9× Blenheim IV, 7× Maryland, 15× Wellington IV
- **Jun (GT35–38):** 5× Beaufighter IF, 15× Blenheim IVF, 5× Gladiator II, 136× Hurricane I, 41× Tomahawk, 10× Blenheim I, 10× Blenheim IV, 6× Maryland, 12× Lysander I, 6× Wellington I. **WD:** 1F, 1B
- **Jul (GT39–42):** 3× Gladiator II, 32× Hurricane I, 20× Tomahawk, 15× Wildcat, 12× Albacore, 35× Blenheim I, 50× Blenheim I(?), 12× Maryland, 10× Wellington I, 16× Wellington IV. **WD:** 1R
- **Aug (GT43–46):** 4× Beaufighter IF, 2× Gladiator II, 27× Hurricane I, 67× Tomahawk, 7× Beaufort I, 22× Blenheim I, 44× Blenheim IV, 30× Maryland, 14× Wellington IV. **WD:** 1B
- **Sep (GT47–50):** 3× Beaufighter IF, 13× Blenheim IVF, 140× Hurricane I, 20× Hurricane IIA, 23× Tomahawk, 2× Beaufort I, 15× Blenheim I, 10× Blenheim IV, 3× Maryland, 13× Wellington IV. **WD:** 1F\*
- **Oct (GT51–54):** 8× Beaufighter IF, 15× Fulmar, 81× Hurricane I, 16× Tomahawk, 3× B17D, 3× Beaufort I, 10× Blenheim I, 26× Blenheim IV, 7× Boston III, 2× Maryland, 15× Wellington I
- **Nov (GT55–58):** 1× Beaufighter IF, 40× Hurricane I, 20× Hurricane IIA, 62× Kittyhawk I, 1× B17D, 4× Baltimore I, 4× Beaufort I, 16× Blenheim IV, 1× Bombay, 23× Boston III, 2× Maryland, 17× Wellington I
- **Dec (GT59–62):** 32× Beaufighter IF, 61× Hurricane I, 32× Hurricane IIA, 90× Kittyhawk I, 45× Kittyhawk II, 9× Baltimore I, 1× Beaufort I, 40× Blenheim I, 20× Blenheim IV, 32× Boston III, 3× Maryland, 1× Tiger Moth II, 10× Wellington I, 9× Wellington IV. **WD:** 2F, 1B
- **1942 Jan (GT63–66):** 3× Beaufighter IF, 15× Blenheim IVF, 96× Hurricane I, 54× Hurricane IIA, 36× Hurricane IIB, 56× Kittyhawk I, 45× Kittyhawk II, 15× Kittyhawk III, 4× Tomahawk, 1× Caudron, 4× B17D, 4× B24, 15× Baltimore I, 2× Beaufort I, 37× Blenheim IV, 7× Boston III, 3× Hudson, 3× Lysander, 18× Wellington IV. **WD:** 4B\*\*
- **Feb (GT67–70):** 21× Beaufighter IF, 7× Blenheim IVF, 15× Hurricane I, 36× Hurricane IIA, 21× Hurricane IIB, 16× Hurricane IIC, 30× Kittyhawk II, 12× Kittyhawk III, 30× Baltimore I, 6× Beaufort I, 4× Boston III, 2× Maryland, 2× Wellington I, 30× Wellington IV. **WD:** 2F, 2B‡
- **Mar (GT71–74):** 25× Beaufighter IF, 18× Hurricane I, 3× Hurricane IIA, 38× Hurricane IIB, 22× Hurricane IIC, 30× Kittyhawk I, 48× Kittyhawk II, 30× Kittyhawk III, 35× Spitfire VB, 12× Anson, 46× Baltimore I, 12× Beaufort I, 10× Boston III, 10× Hudson, 1× Sunderland, 30× Wellington I, 60× Wellington IV
- **Apr (GT75–78):** 13× Beaufighter IF, 5× Beaufighter VI, 13× Hurricane IID, 36× Hurricane IIC, 11× Kittyhawk II, 61× Spitfire VB, 2× Caudron, 4× Anson I, 50× Baltimore I, 5× Beaufort I, 2× Boston III, 13× Hudson, 2× Potez, 16× Wellington I, 45× Wellington IV
- **May (GT79–82):** 3× Beaufighter IF, 17× Beaufighter VI, 13× Hurricane IID, 16× Kittyhawk I, 40× Kittyhawk III, 96× Spitfire VB, 3× Tomahawk, 2× Dewoitine, 22× Anson I, 25× B17D, 25× B24, 42× Baltimore I, 10× Beaufort I, 23× Blenheim IV, 42× Hudson, 1× Maryland, 57× Wellington IV. **WD:** 1F
- **Jun (GT83–86):** 31× Beaufighter VI, 18× Hurricane IIA, 36× Hurricane IIB, 50× Hurricane IIC, 16× Kittyhawk II, 59× Spitfire VB, 14× Spitfire VC, 26× Baltimore I, 42× Blenheim IV, 4× Beaufort I, 5× Boston III, 4× Hudson
- **Jul (GT87–90):** 4× Beaufighter IF, 28× Beaufighter VI, 18× Hurricane IIA, 30× Hurricane IIB, 30× Hurricane IIC, 23× Kittyhawk I, 22× Spitfire VB, 59× Spitfire VC, 4× Anson I, 2× B24, 8× Baltimore I, 12× Beaufort I, 136× Blenheim IV, 6× Wellington I, 18× Wellington IV
- **Aug (GT91–94):** 49× Beaufighter VI, 35× Hurricane IIA, 12× Hurricane IIB, 36× Hurricane IIC, 36× Kittyhawk II, 43× Kittyhawk III, 30× Spitfire VB, 42× Spitfire VC, 1× Dewoitine, 10× Anson I, 15× B24, 57× B25(?), 19× B26, 22× Beaufort I, 101× Blenheim IV, 2× Halifax II, 83× Hudson, 1× Tiger Moth II, 35× Wellington IV. **WD:** 2F\*, 1B
- **Sep (GT95–98):** 1× Beaufighter IF, 51× Beaufighter VI, 20× Hurricane I, 45× Hurricane IIA, 40× Hurricane IIB, 58× Hurricane IIC, 30× Kittyhawk I, 67× Kittyhawk II, 50× Kittyhawk III, 15× Spitfire VB, 35× Spitfire VC, 2× Caudron, 12× Anson I, 1× B24, 30× B26, 9× Baltimore I, 11× Beaufort I, 51× Blenheim IV, 40× Hudson, 1× Maryland, 2× Wellington I, 30× Wellington IV
- **Oct (GT99–102):** 36× Beaufighter VI, 30× Hurricane IIB, 18× Hurricane IIC, 15× Hurricane IID, 18× Kittyhawk II, 40× Kittyhawk III, 15× Spitfire VB, 15× Spitfire VC, 6× Anson I, 26× B25, 3× Beaufort I, 15× Blenheim IV, 12× Hudson, 18× Wellington IV
- **Nov (GT103–106):** 21× Beaufighter VI, 6× Hurricane IIB, 12× Hurricane IIC, 15× Hurricane IID, 12× Kittyhawk III, 30× Spitfire VC, 6× Anson I, 10× B25, 18× B26, 15× Blenheim IV, 5× Hudson. **WD:** 2F\*
- **Dec (GT107–110):** 2× Beaufighter VI, 24× Hurricane IIB, 18× Hurricane IIC, 12× Kittyhawk III, 15× Spitfire VC, 15× Blenheim IV, 8× Wellington IV

---

## §34.85 — Axis (German `Ge:` / Italian `It:`)

- **1940 Sep IV (GT2):** It: 12× Ba65
- **Oct (GT3–6):** It: 12× Ba65
- **Nov (GT7–10):** It: 16× CR42
- **Dec (GT11–14):** It: 40× CR42, 24× G50, 24× Ba65, 3× Z501, 2× Ro37bis, 53× SM79
- **1941 Jan (GT15–18):** It: 36× CR42, 12× G50, 12× Ba65, 12× SM79
- **Feb (GT19–22):** Ge: 30× Bf110, 15× FW200C, 30× Ju87B, 36× Ju88D. It: 12× CR42, 24× G50, 6× Ba65, 6× Z501, 6× Z1007, 12× Ro37bis
- **Mar (GT23–26):** Ge: 32× Bf110, 12× Hs126, 15× Ju87B. It: 36× CR42, 36× G50, 12× Ba65, 12× BR20, 6× Z501, 6× Z1007, 12× SM79
- **Apr (GT27–30):** Ge: 46× Bf109E, 12× Bf110, 12× Ju52, 15× Ju87B, 12× Ju88D. It: 6× CR42, 6× C200, 24× G50, 6× Ba65, 12× BR20, 12× Ro37bis, 18× SM79
- **May (GT31–34):** Ge: 42× Bf109E, 18× Bf110, 15× He111, 15× Ju52, 24× Ju87B, 15× Ju88D. It: 12× CR42, 12× CR42AS, 12× C200, 6× Ba65, 12× BR20, 18× Ca311, 6× Z501, 12× Ro37bis, 12× SM79, 12× SM81
- **Jun (GT35–38):** Ge: 36× Bf109E, 15× He111, 12× Ju87B, 15× Ju88D. It: 12× CR42, 30× C200, 24× G50, 6× BR20, 12× Ca311, 6× Z501, 24× SM79
- **Jul (GT39–42):** Ge: 40× Bf109E, 15× Bf109F, 6× Bf110, 15× Ju52. It: 24× CR42, 48× C200, 6× G50, 12× Ba65, 12× BR20, 6× Ca311, 12× SM79
- **Aug (GT43–46):** Ge: 38× Bf109E, 18× Bf109F, 15× Bf110, 24× Ju87B, 18× Ju88D. It: 12× CR42, 24× C200, 12× G50, 24× BR20, 12× Ca311, 18× SM79\*
- **Sep (GT47–50):** Ge: 42× Bf109E, 32× Bf109F, 28× Bf110, 12× FW200C, 18× He111, 10× Ju87B, 12× Ju87D, 30× Ju88D. It: 36× CR42, 6× CR42AS, 24× C200, 24× G50, 12× BR20, 12× Ca311, 12× SM79
- **Oct (GT51–54):** Ge: 56× Bf109E, 42× Bf109F, 28× Bf110, 12× Arado196, 12× He111, 18× Ju52, 12× Ju87D, 18× Ju88D. It: 24× CR42, 12× C200, 6× G50, 12× Re2001, 12× BR20, 6× SM79, 6× SM81
- **Nov (GT55–58):** Ge: 50× Bf109E, 30× Bf109F, 6× Bf110, 15× Ju88C, 12× Ju52, 18× Ju87D, 18× Ju88D. It: 12× CR42, 6× CR42AS, 18× C200, 60× C202, 6× G50, 24× Re2001, 12× BR20, 6× Ca311, 36× SM79†
- **Dec (GT59–62):** Ge: 18× Bf109E, 38× Bf109F, 15× Bf110, 30× Ju87D, 6× Ju88D. It: 36× CR42, 48× C200, 60× C202, 6× Re2001, 18× BR20, 12× Z506B, 12× SM79
- **1942 Jan (GT63–66):** Ge: 12× Bf109E, 48× Bf109F, 15× Bf110, 15× Ju88C, 6× He111, 12× Ju52, 15× Ju87D, 6× Ju88D. It: 12× CR42, 40× C200, 18× Re2001, (?)× BR20, 12× SM79
- **Feb (GT67–70):** Ge: 36× Bf109E, 12× Bf109F, 15× Bf110, 18× Ju88C, 18× He111, 30× Ju87D, 12× Ju88D. It: 24× CR42, 36× C200, 12× C202, 12× G50, 12× Re2001, 6× BR20, 6× Ca311
- **Mar (GT71–74):** Ge: 9× Bf109E, 45× Bf109F, 15× Bf110, 12× Ju52, 15× Ju87D, 12× Ju88D. It: 24× CR42, 12× C200, 24× C202, 12× G50, 12× Re2001, 6× SM79\*
- **Apr (GT75–78):** Ge: 36× Bf109F, 40× Bf109G, 12× Bf110, 18× Ju88C, 6× Arado196, 6× Ju87D. It: 12× CR42, 6× CR42AS, 24× C200, 24× C202, 24× Re2001, 6× BR20, 12× SM79, 12× SM84†
- **May (GT79–82):** Ge: 30× Bf109F, 36× Bf109G, 12× FW200C, 12× He111, 18× Ju52, 24× Ju87D, 30× Ju88D. It: 24× CR42, 24× C200, 36× C202, 12× G50, 6× Re2001, 12× Z1007bis, 6× SM79, 6× SM84\*
- **Jun (GT83–86):** Ge: 15× Bf109F, 48× Bf109G, 12× He111, 15× Ju52, 10× Ju87D, 30× Ju88D. It: 12× CR42, 6× CR42AS, 18× C200, 12× C202, 18× G50, 6× Re2001, 6× Ca311, 12× Z1007bis, 6× SM84
- **Jul (GT87–90):** Ge: 20× Bf109F, 18× Bf109G, 12× Ju88C, 12× Ju52, 12× Ju87D. It: 18× CR42, 12× C200, 36× C202, 6× G50, 18× Re2001, 12× Z1007bis, 6× SM79, 12× SM84
- **Aug (GT91–94):** Ge: 36× Bf109G, 12× Ju87D, 15× Ju88D. It: 18× CR42, 12× C200, 24× C202, 6× G50, 6× SM79, 12× SM84
- **Sep (GT95–98):** Ge: 45× Bf109G, 18× Ju52, 6× Ju87D, 12× Ju88D. It: 36× CR42, 6× CR42AS, 6× C200, 36× C202, 12× G50, 6× Re2001, 12× Z1007bis, 12× SM79†
- **Oct (GT99–102):** Ge: 12× Bf109G, 12× He111, 12× Ju87D, 6× Ju88D. It: 24× CR42, 6× CR42AS, 12× C200, 48× C202, 6× G50, 6× SM79
- **Nov (GT103–106):** Ge: 12× Bf109G, 8× Ju87D. It: 36× CR42, 24× C202
- **Dec (GT107–110):** It: 12× CR42, 6× C200, 12× C202

**Footnotes:** `*` six of these S.M.79s were operated as torpedo bombers; `†` twelve of these S.M.79s
were torpedo-bombers. `(?)` = glyph unclear in the source (Jan-1942 Italian BR20 count).
