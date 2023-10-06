#include "Bindings.h"

#include <QtCore/QFile>
#include <QtGui/QGuiApplication>
#include <QtQml/QQmlApplicationEngine>
#include <QtQml/qqml.h>
#include <QQuickStyle>


extern "C" {
    int main_cpp(const char* appPath, quint64 library, quint64 plm) {
        int argc = 1;
        char* argv[1] = { (char*)appPath };
        QGuiApplication app(argc, argv);

        QQuickStyle::setStyle("org.kde.desktop");

        Albums* albums = new Albums(NULL);
        albums->set_library(library);
        qmlRegisterSingletonInstance<Albums>("io.github.mullr.tinysonic", 1, 0, "Albums", albums);

        Player* player = new Player(NULL);
        player->set_library(library);
        player->set_plm(plm);
        qmlRegisterSingletonInstance<Player>("io.github.mullr.tinysonic", 1, 0, "Player", player);

        QQmlApplicationEngine engine;
        engine.load(QUrl(QStringLiteral("ui/main.qml")));

        if (engine.rootObjects().isEmpty()) {
            return -1;
        }

        return app.exec();
    }
} 
