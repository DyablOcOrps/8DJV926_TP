# TP Réseau - MMORPG Architecture

Ce projet implémente une architecture de serveur réseau pour MMO, comprenant un Gatekeeper, un Orchestrateur et des serveurs dédiés dynamiques, le tout synchronisé via Redis.

## Prérequis

- [Docker](https://www.docker.com/) et Docker Compose installés sur votre machine.

## Démarrage rapide

Pour lancer l'ensemble des composants (Redis, l'Orchestrateur et le Gatekeeper) en une seule commande, exécutez :

```bash
docker-compose up

 ```

si jamais l'orchestrator ne lance pas les serveur, taper cette commande dans un autre terminal:

```bash
docker exec -it mmorpg-orchestrator cargo build --bin dedicated_server