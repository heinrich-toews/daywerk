# Anforderungen

## Aufgabe der App

Die App erfasst die Arbeitszeiten der Mitarbeiter einer Firma oder eines Vereins (Stempel-App) für ein bestimmtes Zeitkonto.

Die Default Zeitkonten sind "ALLGEMEIN" und "PAUSE".

Per Default sind 3 große Buttons zu sehen: "ALLGEMEIN" (mit Play-Symbol), dann "PAUSE" (mit Pause-Symbol, ausgegraut zu Beginn), und "STOP" (mit STOP-Symbol, ausgegraut zu Beginn). Diesee Buttons stehen auf einer Zeile nebeneinander. Dadrunter erscheinen weitere Buttons, eins je Zeitkonto auch mit einem Play-Symbol. Diese sind NICHT ausgegraut. Der Benutzer kann nun auf einen der PLAY-Buttons drücken um die Zeitmessung zu starten. Die Zeit wird dabei auf das jeweilige Zeitkonto gebucht zu dem der Button gehört (z.B. ALLGEMEIN). Der Aktv-Zustand des Buttons wird deutlich sichtbar gemacht. Wird jetzt direkt auf einen anderen Play-Button (z.B. Kunde TWx) gedrückt, so stoppt die Zeitmessung für das aktuelle Zeitkonto/Button (z.B. ALLGEIMEIN) und die Messung für das neue Zeitkonto/Button (z.B. Kunde TWx) beginnt. Ein Drücken auf PAUSE stoppt die Messung für das aktuell aktive Zeitkonto und beginnt die Messung auf das Zeitkonto PAUSE. Der PAUSE-Button wird aktiv, der andere wird deaktiviert. Hier die Transitionen:

A) Drücken auf Play-Button "ALLGEMEIN" (Start Zeiterfassung für Allgemein) -> Drücken auf Play-Button "Kunde TWx" (Start Zeiterfassung für "Kunde TWx") -> Drücken auf PAUSE-Button (Start Zeiterfassung für PAUSE-Konto) -> Drücken auf Play-Button ALLGEMEIN (Start Zeiterfassung ALLGEIMEN). Bei Wechsel wird jeweils der Stoppzeitpunkt für das zuvor aktive Zeitkonto erfasst. Es gibt mehrere Arbeitszeitkonten und nur ein Pausen-Zeitkonto.

Wird auf STOP Button gedrückt endet damit die Zeiterfassung für den Tag und der Stoppzeitpunkt für das zuletzt aktive Zeitkonto wird erfasst. Befindet sich der Mitarbeiter in der Pause und es wird auf Stop gedrückt, so wird die Pause gelöscht und als Endzeitpunkt der Beginn der Pause für das zuletzt aktive Zeitkonto eingetragen. 

## Datenmodell

- Organisationen (beliebige Anzahl)
- Mitarbeiter (beliebige Anzahl) und Rollen (Allgemein, Büro, Admin)
- Es gibt N Mitarbeiter, die einer Organisation zugeordnet werden. Jeder Mitarbeiter muss teil einer Organisation sein.
- Arbeitszeitkonten (Allgemein, Pause, weitere Konten (=Projekte) angelegt durch den Admin)

## Aufbau

- Modernstes Client-Server Modell
- Server Version ist per Browser erreichbar (https://time.twxsoft.de). Mitarbeiter können sich dort einloggen und buchen. Mobile-First.
- Client-Apps synchronisieren sich mit dem Server per REST-API (moderne und schnelle Technik) (Android/iPhone Apps)
- Client App muss sich auf dem Server einloggen bzw. authentifizieren und synchronisiert ihre Daten bei jeder Änderung mit dem Server (push)
- Server- und Client-Apps können ein Branding einstellen für eine Firma/Organisation (Logo und Farben). Standard-Design modern und neutral.

## Rollen

- Allgemein: Eigene Zeiten erfassen und eigene Statistiken anzeigen.
- Büro: Wie Allgemein, aber zusätzlich Statistiken aller Mitarbeiter/Organisationen abrufen, downloaden (xls,pdf).
- Admin: Wie Büro, aber zusätzlich: Einstellungen vornehmen für Branding für jede Organisation. Benutzer verwalten. Backups erstellen, Daten importieren. SMTP E-Mail Einstellungen konfigurieren.

## Design

- sehr modern
- sehr übersichtlich
- handlich auf dem Smartphone, um Zeiten einfach zu buchen

## Technik

- Angelehnt an /home/twx/projects/twx/git/openchordbook
- Design (Text, Farben, Aufbau): /home/twx/Dokumente/zeiterfassung-quellcode_v3

