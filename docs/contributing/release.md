# Publier une release

Process complet pour publier une nouvelle version de RL Stats Overlay sur GitHub Releases avec **MAJ in-app automatique** pour les utilisateurs existants.

## Prérequis (one-shot)

À faire **une seule fois** par mainteneur, avant la toute première release signée.

### 1. Générer la paire de clés Tauri

```powershell
New-Item -ItemType Directory -Force "$env:USERPROFILE\.tauri-keys" | Out-Null
bun tauri signer generate -w "$env:USERPROFILE\.tauri-keys\rl-stats-overlay.key"
```

La commande te demande un **mot de passe** (laisse vide si tu n'en veux pas, mais c'est moins safe). Note-le, **tu en auras besoin pour CI**.

Deux fichiers sont créés :
- `~/.tauri-keys/rl-stats-overlay.key` — clé **privée** (ne JAMAIS commit, JAMAIS publier)
- `~/.tauri-keys/rl-stats-overlay.key.pub` — clé **publique** (déjà dans `tauri.conf.json:pubkey`)

> 💾 **Backup obligatoire** : copie le `.key` privé sur un disque externe / cloud chiffré. Si tu le perds, tu casses la chaîne d'update à vie — les utilisateurs devront réinstaller à la main pour repartir sur une nouvelle clé.

### 2. Stocker les secrets dans GitHub

Dans le repo : **Settings → Secrets and variables → Actions → New repository secret** :

| Nom du secret | Valeur |
|---|---|
| `TAURI_SIGNING_PRIVATE_KEY` | Le **contenu complet** du fichier `.key` (base64). Récupère-le avec `Get-Content ~/.tauri-keys/rl-stats-overlay.key | Set-Clipboard`. |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Le mot de passe que tu as tapé pendant `signer generate` (laisse vide si tu n'en as pas mis). |

## Process release (à chaque version)

### 1. Bumper la version

Trois fichiers à mettre à jour avec le **même numéro** :

- `package.json` → champ `"version"`
- `src-tauri/Cargo.toml` → champ `version` sous `[package]`
- `src-tauri/tauri.conf.json` → champ `"version"`

```powershell
# Exemple pour 0.1.7 — édite les 3 fichiers à la main, ou utilise un script.
```

### 2. Commit + tag + push

```bash
git add package.json src-tauri/Cargo.toml src-tauri/tauri.conf.json
git commit -m "chore: bump version to 0.1.7"
git tag v0.1.7
git push origin main --tags
```

### 3. Le workflow `release.yml` se déclenche

Sur le push du tag `v0.1.7`, GitHub Actions lance automatiquement :

1. **Build** Windows release (Rust + frontend, ~5-10 min).
2. **Sign** les artefacts (`.exe` + `.msi`) avec la clé privée des secrets.
3. **Crée la GitHub Release** `RL Stats Overlay v0.1.7`.
4. **Upload** le `.exe`, le `.msi`, le `.exe.sig` + un fichier `latest.json` qui contient la version + l'URL + la signature.
5. Le `latest.json` est accessible à `https://github.com/kevindjf/rl-stats-overlay/releases/latest/download/latest.json` — c'est ce que les apps utilisateur interrogent au lancement.

> ⏱️ **Durée totale** : ~10 min entre le `git push` et la dispo de la MAJ pour les utilisateurs.

### 4. Vérifier

- **Page Releases** GitHub : la release `v0.1.7` doit lister 4 fichiers (`.exe`, `.msi`, `.exe.sig`, `latest.json`).
- **Manifeste** : ouvre `https://github.com/kevindjf/rl-stats-overlay/releases/latest/download/latest.json` dans le navigateur — tu dois voir un JSON propre avec `version: "0.1.7"` et un `signature: "..."` non vide.
- **App existante** : lance une instance de la version précédente (ex: 0.1.6) — au bout de 3 secondes, la **bannière "🔔 Nouvelle version 0.1.7 disponible"** doit apparaître. Clique **Installer** → l'app doit télécharger, redémarrer, et afficher la nouvelle version.

## Cas d'erreur

### Le workflow `release.yml` échoue avec "signing key invalid"

Vérifie que :
- `TAURI_SIGNING_PRIVATE_KEY` contient le **fichier entier** (commence par `untrusted comment: minisign secret key:` et plusieurs lignes base64).
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` correspond au password utilisé lors de `signer generate` (vide si pas de password).

### La bannière n'apparaît pas dans l'app

Vérifie dans le DevTools (clic droit sur la fenêtre Settings → Inspect → Console) :
- Erreur `Update check failed: signature verification failed` → la pubkey dans `tauri.conf.json` ne correspond pas à la clé privée utilisée par CI. Re-vérifie que les deux fichiers (`.key` et `.key.pub`) viennent du **même `signer generate`**.
- Erreur `Update check failed: 404` → `latest.json` n'a pas été publié. Vérifie la page Releases.
- Pas d'erreur, juste rien : ta version locale est peut-être déjà à jour, ou la version dans `tauri.conf.json` du build est >= au tag publié.

### Les utilisateurs sur 0.1.6 ne reçoivent pas la bannière

C'est normal : les versions **antérieures à l'introduction de l'updater** n'ont pas de code qui interroge `latest.json`. La 0.1.7 est la **première version qui active la chaîne** — les utilisateurs 0.1.6 doivent télécharger 0.1.7 manuellement depuis GitHub Releases. À partir de la 0.1.7, toutes les MAJ futures (0.1.8, 0.2.0, etc.) sont auto-notifiées.

Pour pousser les utilisateurs 0.1.6 à passer à 0.1.7 : annonce dans le README, sur Discord / Twitter, et dans les release notes.

## Convention de versioning

[SemVer](https://semver.org/) :
- `MAJOR.MINOR.PATCH`
- `0.X.Y` tant que l'app est en pre-1.0 (rupture acceptable entre `MINOR`).
- Tag obligatoirement préfixé `v` : `v0.1.7`, pas `0.1.7` (le workflow filtre sur `v*`).

## Préreleases (rc, beta)

Tagger avec un suffixe :

```bash
git tag v0.2.0-rc.1
git push origin v0.2.0-rc.1
```

Le workflow détecte le suffixe et publie la release **comme `prerelease: true`** — invisible dans `latest.json` (les users stables ne sont pas notifiés). Pour tester sur ton compte, tu télécharges manuellement le `.exe` de la prerelease.
