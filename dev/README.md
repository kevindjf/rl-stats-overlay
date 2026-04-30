# dev/ — outils de développement

Ce dossier sert uniquement aux **contributeurs** du projet. Il n'est pas inclus dans le build de production de l'app.

## Mock Stats API server

Reproduit le WebSocket officiel de Rocket League (`ws://localhost:49123`) et permet de piloter un faux match depuis un panneau de contrôle web. Indispensable pour développer et tester les overlays sans avoir Rocket League lancé (utile sur macOS / Linux notamment).

### Lancer

```bash
bun run dev/mock-server.ts
```

Puis ouvre dans ton navigateur :

- **Control panel** : <http://localhost:49123/control> — démarre un match, ajoute des goals, finis en Win/Loss
- **Boost overlay** : <http://localhost:49123/overlays/boost.html>

Le mock écoute sur **49123**, le même port que la vraie Stats API du jeu, donc les overlays utilisent la même URL WebSocket en dev qu'en prod — zéro changement de configuration.

### Workflow type

1. `bun run dev/mock-server.ts`
2. Ouvre l'overlay dans un onglet
3. Ouvre le control panel dans un autre onglet
4. Clique **▶ Démarrer un match** → la connexion s'établit, l'overlay devient interactif
5. Spamme **+1 Goal**, **+1 Save** → animations live
6. Clique **🏆 Finir — Win** ou **💀 Finir — Loss** → la session W/L/streak s'incrémente

## Couplage avec l'app Tauri

Pendant le développement de l'app Tauri principale :

- Le mock server tient la place de Rocket League (port 49123, WebSocket)
- L'app Tauri tourne en parallèle (port 49124, HTTP) et héberge les overlays + l'UI settings
- Les overlays connectés à `ws://localhost:49123` reçoivent les événements du mock comme s'ils venaient du jeu

Cela permet d'itérer sur l'app sans avoir une instance Rocket League sur la machine de dev.
