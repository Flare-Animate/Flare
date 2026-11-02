// GameEngine.cpp - Core game engine implementation
// Copyright (c) 2026 Flare Project

#include "GameEngine.h"

#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonArray>
#include <QFile>
#include <QDir>
#include <QStandardPaths>
#include <QTimer>
#include <QElapsedTimer>
#include <QApplication>
#include <QMouseEvent>
#include <QKeyEvent>
#include <algorithm>
#include <cmath>

namespace TGame {

//------------------------------------------------------------------
// Component implementations
//------------------------------------------------------------------

ComponentID registerComponentType(const char* name) {
    static QMap<QString, ComponentID> s_types;
    static ComponentID s_next = 1;  // 0 reserved

    QString key(name);
    auto it = s_types.find(key);
    if (it != s_types.end()) return *it;

    ComponentID id = s_next++;
    s_types[key] = id;
    return id;
}

const char* getComponentName(ComponentID id) {
    static QMap<ComponentID, QString> s_names;
    static bool initialized = false;
    if (!initialized) {
        initialized = true;
        s_names[1] = "Transform";
        s_names[2] = "Sprite";
        s_names[3] = "PhysicsBody";
        s_names[4] = "Script";
        s_names[5] = "Audio";
        s_names[6] = "Camera";
        s_names[7] = "UI";
        s_names[8] = "Particle";
    }
    auto it = s_names.find(id);
    return it != s_names.end() ? it.value().toStdString().c_str() : "Unknown";
}

Component* TransformComponent::clone() const {
    auto* c = new TransformComponent(*this);
    return c;
}

Component* SpriteComponent::clone() const {
    auto* c = new SpriteComponent(*this);
    return c;
}

Component* PhysicsBodyComponent::clone() const {
    auto* c = new PhysicsBodyComponent(*this);
    c->bodyPtr = nullptr;  // runtime pointer not cloned
    return c;
}

Component* ScriptComponent::clone() const {
    auto* c = new ScriptComponent(*this);
    return c;
}

Component* AudioComponent::clone() const {
    auto* c = new AudioComponent(*this);
    c->soundHandle = nullptr;
    return c;
}

Component* CameraComponent::clone() const {
    auto* c = new CameraComponent(*this);
    return c;
}

Component* UIComponent::clone() const {
    auto* c = new UIComponent(*this);
    return c;
}

Component* ParticleComponent::clone() const {
    auto* c = new ParticleComponent(*this);
    c->emitterHandle = nullptr;
    return c;
}

//------------------------------------------------------------------
// Entity implementation
//------------------------------------------------------------------

template<typename T>
constexpr ComponentID T::staticType;

//------------------------------------------------------------------
// TransformSystem
//------------------------------------------------------------------

TransformSystem::TransformSystem() {
    setRequiredMask((EntityMask(1) << TransformComponent::staticType));
    setPriority(100);  // run first
}

void TransformSystem::update(double dt) {
    if (!scene_) return;

    auto entities = scene_->query<TransformComponent>();
    for (Entity* e : entities) {
        TransformComponent* t = e->getComponent<TransformComponent>();
        if (!t || !t->active) continue;

        // Parent transform propagation would go here
        // For now just mark as clean
    }
}

//------------------------------------------------------------------
// SpriteSystem
//------------------------------------------------------------------

SpriteSystem::SpriteSystem() {
    setRequiredMask(
        (EntityMask(1) << TransformComponent::staticType) |
        (EntityMask(1) << SpriteComponent::staticType)
    );
    setPriority(200);
}

void SpriteSystem::update(double dt) {
    if (!scene_) return;

    auto entities = scene_->query<TransformComponent, SpriteComponent>();
    for (Entity* e : entities) {
        SpriteComponent* s = e->getComponent<SpriteComponent>();
        TransformComponent* t = e->getComponent<TransformComponent>();
        if (!s || !s->active || !t) continue;

        // Animation frame advancement
        if (s->frame > 0) {
            // Could advance frame based on animation rate
        }
    }
}

void SpriteSystem::render() {
    if (!scene_) return;

    auto entities = scene_->query<TransformComponent, SpriteComponent>();
    // Sort by renderOrder
    QVector<Entity*> sorted = entities;
    std::sort(sorted.begin(), sorted.end(),
        [](Entity* a, Entity* b) {
            SpriteComponent* sa = a->getComponent<SpriteComponent>();
            SpriteComponent* sb = b->getComponent<SpriteComponent>();
            return sa->renderOrder < sb->renderOrder;
        });

    for (Entity* e : sorted) {
        SpriteComponent* s = e->getComponent<SpriteComponent>();
        TransformComponent* t = e->getComponent<TransformComponent>();
        if (!s || !s->active || !s->visible || !t) continue;

        // Actual rendering would use Flare's renderer
        // This is where we'd draw the sprite with transform
    }
}

//------------------------------------------------------------------
// PhysicsSystem
//------------------------------------------------------------------

PhysicsSystem::PhysicsSystem() {
    setRequiredMask(
        (EntityMask(1) << TransformComponent::staticType) |
        (EntityMask(1) << PhysicsBodyComponent::staticType)
    );
    setPriority(150);  // after transform, before render
}

PhysicsSystem::~PhysicsSystem() {
    if (world_) {
        // Cleanup Box2D world
    }
}

void PhysicsSystem::init(Scene* scene) {
    System::init(scene);
    // Initialize Box2D world
    // b2Vec2 gravity(0, -9.8 * worldScale_);
    // world_ = new b2World(gravity);
}

void PhysicsSystem::update(double dt) {
    if (!scene_ || !world_) return;

    // Step physics
    const int velocityIterations = 8;
    const int positionIterations = 3;
    // world_->Step(dt, velocityIterations, positionIterations);

    // Sync transforms from physics bodies
    auto entities = scene_->query<TransformComponent, PhysicsBodyComponent>();
    for (Entity* e : entities) {
        PhysicsBodyComponent* p = e->getComponent<PhysicsBodyComponent>();
        TransformComponent* t = e->getComponent<TransformComponent>();
        if (!p || !p->active || !p->bodyPtr || !t) continue;

        // Sync from Box2D
        // b2Body* body = static_cast<b2Body*>(p->bodyPtr);
        // b2Vec2 pos = body->GetPosition();
        // t->x = pos.x * worldScale_;
        // t->y = pos.y * worldScale_;
        // t->rotation = body->GetAngle();
    }
}

void PhysicsSystem::setGravity(double x, double y) {
    if (world_) {
        // world_->SetGravity(b2Vec2(x, y));
    }
}

void PhysicsSystem::setWorldScale(double pixelsPerMeter) {
    worldScale_ = pixelsPerMeter;
}

void PhysicsSystem::setCollisionCallback(CollisionCallback cb) {
    collisionCallback_ = std::move(cb);
}

//------------------------------------------------------------------
// ScriptSystem
//------------------------------------------------------------------

ScriptSystem::ScriptSystem() {
    setRequiredMask(
        (EntityMask(1) << TransformComponent::staticType) |
        (EntityMask(1) << ScriptComponent::staticType)
    );
    setPriority(100);
}

void ScriptSystem::init(Scene* scene) {
    System::init(scene);
    // Initialize Lua/JS/AVM2 context
}

void ScriptSystem::update(double dt) {
    if (!scene_) return;

    auto entities = scene_->query<ScriptComponent>();
    for (Entity* e : entities) {
        ScriptComponent* s = e->getComponent<ScriptComponent>();
        if (!s || !s->active) continue;

        // Execute script logic
    }
}

void ScriptSystem::loadScript(const TFilePath& path) {
    // Load and compile script
}

void ScriptSystem::callFunction(const QString& funcName, const QVector<QJsonValue>& args) {
    // Call script function
}

//------------------------------------------------------------------
// AudioSystem
//------------------------------------------------------------------

AudioSystem::AudioSystem() {
    setRequiredMask(
        (EntityMask(1) << TransformComponent::staticType) |
        (EntityMask(1) << AudioComponent::staticType)
    );
    setPriority(300);
}

void AudioSystem::init(Scene* scene) {
    System::init(scene);
}

void AudioSystem::update(double dt) {
    if (!scene_) return;

    auto entities = scene_->query<TransformComponent, AudioComponent>();
    for (Entity* e : entities) {
        AudioComponent* a = e->getComponent<AudioComponent>();
        TransformComponent* t = e->getComponent<TransformComponent>();
        if (!a || !a->active) continue;

        // Update 3D audio position if spatial
        if (a->spatial && a->soundHandle && t) {
            // Update 3D audio position
        }
    }
}

void AudioSystem::playSound(const TFilePath& path, double volume, bool loop) {
    // Play sound via Flare's audio system
}

void AudioSystem::stopAll() {
    // Stop all sounds
}

void AudioSystem::setMasterVolume(double v) {
    masterVolume_ = std::clamp(v, 0.0, 1.0);
}

//------------------------------------------------------------------
// CameraSystem
//------------------------------------------------------------------

CameraSystem::CameraSystem() {
    setRequiredMask(
        (EntityMask(1) << TransformComponent::staticType) |
        (EntityMask(1) << CameraComponent::staticType)
    );
    setPriority(250);
}

void CameraSystem::update(double dt) {
    if (!scene_) return;

    auto entities = scene_->query<CameraComponent>();
    for (Entity* e : entities) {
        CameraComponent* c = e->getComponent<CameraComponent>();
        if (!c || !c->active) continue;

        if (c->followEntity && c->followTarget != INVALID_ENTITY) {
            Entity* target = scene_->getEntity(c->followTarget);
            if (target) {
                TransformComponent* t = target->getComponent<TransformComponent>();
                if (t) {
                    // Smooth camera follow
                    // c->x = lerp(c->x, t->x, c->lerpSpeed * dt);
                    // c->y = lerp(c->y, t->y, c->lerpSpeed * dt);
                }
            }
        }

        // Clamp to bounds
        if (!c->bounds.isEmpty()) {
            // clamp
        }
    }
}

void CameraSystem::setViewport(double w, double h) {
    if (!scene_) return;
    auto entities = scene_->query<CameraComponent>();
    for (Entity* e : entities) {
        CameraComponent* c = e->getComponent<CameraComponent>();
        if (c) {
            c->viewportWidth = w;
            c->viewportHeight = h;
        }
    }
}

QTransform CameraSystem::worldToScreen() const {
    if (!scene_) return QTransform();
    auto entities = scene_->query<CameraComponent>();
    if (entities.empty()) return QTransform();

    CameraComponent* c = entities.first()->getComponent<CameraComponent>();
    if (!c) return QTransform();

    QTransform t;
    t.translate(c->viewportWidth / 2, c->viewportHeight / 2);
    t.scale(c->zoom, c->zoom);
    return t;
}

QTransform CameraSystem::screenToWorld() const {
    return worldToScreen().inverted();
}

QPointF CameraSystem::screenToWorld(const QPointF& screenPos) const {
    return screenToWorld().map(screenPos);
}

QPointF CameraSystem::worldToScreen(const QPointF& worldPos) const {
    return worldToScreen().map(worldPos);
}

//------------------------------------------------------------------
// ParticleSystem
//------------------------------------------------------------------

ParticleSystem::ParticleSystem() {
    setRequiredMask(
        (EntityMask(1) << TransformComponent::staticType) |
        (EntityMask(1) << ParticleComponent::staticType)
    );
    setPriority(200);
}

void ParticleSystem::update(double dt) {
    if (!scene_) return;

    auto entities = scene_->query<ParticleComponent>();
    for (Entity* e : entities) {
        ParticleComponent* p = e->getComponent<ParticleComponent>();
        if (!p || !p->active) continue;

        // Update emitter
    }
}

void ParticleSystem::render() {
    if (!scene_) return;

    // Render all active emitters
}

void* ParticleSystem::createEmitter(const TFilePath& config, double x, double y) {
    // Create emitter from config
    return nullptr;
}

void ParticleSystem::destroyEmitter(void* handle) {
    // Destroy emitter
}

//------------------------------------------------------------------
// UISystem
//------------------------------------------------------------------

UISystem::UISystem() {
    setRequiredMask(
        (EntityMask(1) << TransformComponent::staticType) |
        (EntityMask(1) << UIComponent::staticType)
    );
    setPriority(400);
}

void UISystem::update(double dt) {
    if (!scene_) return;

    auto entities = scene_->query<UIComponent>();
    for (Entity* e : entities) {
        UIComponent* u = e->getComponent<UIComponent>();
        if (!u || !u->active || !u->interactive) continue;

        // Handle UI interactions
    }
}

void UISystem::render() {
    if (!scene_) return;

    // Render UI elements
}

void UISystem::handleMouseEvent(const QPointF& pos, int button, bool pressed) {
    if (!scene_) return;

    auto entities = scene_->query<UIComponent>();
    for (Entity* e : entities) {
        UIComponent* u = e->getComponent<UIComponent>();
        if (!u || !u->active || !u->interactive) continue;

        // Check hit test
        // if (u->rect.contains(pos)) { ... }
    }
}

void UISystem::handleKeyEvent(int key, bool pressed) {
    // Handle keyboard input for UI
}

void UISystem::handleTextInput(const QString& text) {
    // Handle text input
}

//------------------------------------------------------------------
// Scene implementation
//------------------------------------------------------------------

Scene::Scene(const QString& name) : name_(name) {
    assets_ = std::make_unique<AssetManager>();
    input_ = std::make_unique<InputManager>();
    physics_ = std::make_unique<PhysicsWorld>();
    audio_ = std::make_unique<AudioManager>();
}

Scene::~Scene() {
    shutdown();
}

Entity Scene::createEntity() {
    EntityID id = nextEntityID_++;
    Entity e(id);
    entities_[id] = std::move(e);
    return entities_[id];
}

void Scene::destroyEntity(EntityID id) {
    auto it = entities_.find(id);
    if (it == entities_.end()) return;

    // Notify systems
    for (System* s : systems_) {
        s->onEntityRemoved(&it->second);
    }

    // Clean up components
    for (auto& comp : it->second.components_) {
        delete comp.second;
    }

    entities_.erase(it);
}

Entity* Scene::getEntity(EntityID id) {
    auto it = entities_.find(id);
    return it != entities_.end() ? &it->second : nullptr;
}

const Entity* Scene::getEntity(EntityID id) const {
    auto it = entities_.find(id);
    return it != entities_.end() ? &it->second : nullptr;
}

QVector<Entity*> Scene::query(EntityMask mask) {
    QVector<Entity*> result;
    for (auto& pair : entities_) {
        if ((pair.second.mask() & mask) == mask) {
            result.push_back(&pair.second);
        }
    }
    return result;
}

void Scene::addSystem(System* system) {
    if (!system) return;
    system->setScene(this);
    system->setSystemID(nextSystemID_++);
    systems_.push_back(system);
    std::sort(systems_.begin(), systems_.end(),
        [](System* a, System* b) { return a->priority() > b->priority(); });

    system->init(this);
}

void Scene::removeSystem(System* system) {
    auto it = std::find(systems_.begin(), systems_.end(), system);
    if (it != systems_.end()) {
        systems_.erase(it);
    }
}

System* Scene::getSystem(SystemID type) {
    for (System* s : systems_) {
        if (s->systemID() == type) return s;
    }
    return nullptr;
}

void Scene::init() {
    for (System* s : systems_) {
        s->init(this);
    }
}

void Scene::update(double dt) {
    for (System* s : systems_) {
        s->update(dt);
    }
}

void Scene::render() {
    for (System* s : systems_) {
        s->render();
    }
}

void Scene::shutdown() {
    for (System* s : systems_) {
        delete s;
    }
    systems_.clear();

    for (auto& pair : entities_) {
        for (auto& comp : pair.second.components_) {
            delete comp.second;
        }
    }
    entities_.clear();
}

bool Scene::save(const TFilePath& path) {
    QJsonObject root;
    root["name"] = name_;

    QJsonArray entitiesArray;
    for (auto& pair : entities_) {
        QJsonObject e;
        e["id"] = pair.first;
        e["mask"] = QString::number(pair.second.mask());

        QJsonArray compsArray;
        for (auto& compPair : pair.second.components_) {
            QJsonObject c;
            c["type"] = getComponentName(compPair.first);
            // Serialize component data
            compsArray.append(c);
        }
        e["components"] = compsArray;
        entitiesArray.append(e);
    }
    root["entities"] = entitiesArray;

    QJsonDocument doc(root);
    QFile file(path.getQString());
    if (!file.open(QIODevice::WriteOnly)) return false;
    file.write(doc.toJson(QJsonDocument::Indented));
    return true;
}

bool Scene::load(const TFilePath& path) {
    QFile file(path.getQString());
    if (!file.open(QIODevice::ReadOnly)) return false;

    QJsonDocument doc = QJsonDocument::fromJson(file.readAll());
    if (doc.isNull()) return false;

    QJsonObject root = doc.object();
    name_ = root["name"].toString();

    QJsonArray entitiesArray = root["entities"].toArray();
    for (const auto& val : entitiesArray) {
        QJsonObject e = val.toObject();
        EntityID id = e["id"].toInt();
        Entity e2 = createEntity();
        // Would need to recreate with specific ID
        QJsonArray compsArray = e["components"].toArray();
        for (const auto& cval : compsArray) {
            QJsonObject c = cval.toObject();
            QString typeName = c["type"].toString();
            // Deserialize component
        }
    }
    return true;
}

//------------------------------------------------------------------
// AssetManager implementation
//------------------------------------------------------------------

struct AssetManager::Impl {
    QMap<TFilePath, void*> textureCache;
    QMap<TFilePath, void*> audioCache;
    QMap<TFilePath, void*> scriptCache;
    QMap<TFilePath, void*> particleCache;
    size_t maxCacheSize = 100 * 1024 * 1024;  // 100 MB
};

AssetManager::AssetManager() : impl_(std::make_unique<Impl>()) {}
AssetManager::~AssetManager() { releaseAll(); }

void* AssetManager::loadTexture(const TFilePath& path) {
    auto it = impl_->textureCache.find(path);
    if (it != impl_->textureCache.end()) return it->second;

    // Load via Flare's image IO
    // void* handle = loadImageViaFlare(path);
    // if (handle) impl_->textureCache[path] = handle;
    // return handle;
    return nullptr;
}

void* AssetManager::loadSWFSymbol(const TFilePath& swfPath, const QString& symbolName) {
    // Load symbol from SWF via Flare's SWF reader
    return nullptr;
}

void* AssetManager::loadAudio(const TFilePath& path) {
    auto it = impl_->audioCache.find(path);
    if (it != impl_->audioCache.end()) return it->second;

    // Load via Flare's audio system
    // void* handle = loadAudioViaFlare(path);
    // if (handle) impl_->audioCache[path] = handle;
    // return handle;
    return nullptr;
}

void* AssetManager::loadScript(const TFilePath& path) {
    auto it = impl_->scriptCache.find(path);
    if (it != impl_->scriptCache.end()) return it->second;

    // Load script file
    // void* handle = loadScriptViaFlare(path);
    // if (handle) impl_->scriptCache[path] = handle;
    // return handle;
    return nullptr;
}

void* AssetManager::loadParticleConfig(const TFilePath& path) {
    auto it = impl_->particleCache.find(path);
    if (it != impl_->particleCache.end()) return it->second;

    // Load particle config
    return nullptr;
}

void* AssetManager::loadFont(const TFilePath& path) {
    // Load font
    return nullptr;
}

void AssetManager::release(void* handle) {
    // Remove from caches
}

void AssetManager::releaseAll() {
    impl_->textureCache.clear();
    impl_->audioCache.clear();
    impl_->scriptCache.clear();
    impl_->particleCache.clear();
}

void AssetManager::setCacheSize(size_t bytes) {
    impl_->maxCacheSize = bytes;
}

size_t AssetManager::cacheSize() const {
    // Calculate current cache size
    return 0;
}

//------------------------------------------------------------------
// InputManager implementation
//------------------------------------------------------------------

InputManager::InputManager() {
    // Initialize input state
}

InputManager::~InputManager() {}

void InputManager::update() {
    // Update key/mouse state
}

bool InputManager::isKeyPressed(int key) const {
    // Check key state
    return false;
}

bool InputManager::isKeyJustPressed(int key) const {
    return false;
}

bool InputManager::isKeyJustReleased(int key) const {
    return false;
}

bool InputManager::isMouseButtonPressed(int button) const {
    return false;
}

bool InputManager::isMouseButtonJustPressed(int button) const {
    return false;
}

QPointF InputManager::mousePosition() const {
    return QPointF();
}

QPointF InputManager::mouseDelta() const {
    return QPointF();
}

bool InputManager::isGamepadConnected(int index) const {
    return false;
}

float InputManager::gamepadAxis(int index, int axis) const {
    return 0.0f;
}

bool InputManager::gamepadButton(int index, int button) const {
    return false;
}

QString InputManager::getTextInput() const {
    return QString();
}

void InputManager::clearTextInput() {}

}  // namespace TGame