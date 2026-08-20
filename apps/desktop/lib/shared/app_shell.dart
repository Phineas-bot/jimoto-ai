import 'package:flutter/material.dart';
import 'package:gixgiz_desktop/app/app_keys.dart';
import 'package:gixgiz_desktop/app/app_routes.dart';
import 'package:gixgiz_desktop/l10n/app_localizations.dart';

class AppShell extends StatelessWidget {
  const AppShell({required this.selectedIndex, required this.child, super.key});

  static const _railBreakpoint = 720.0;

  final int selectedIndex;
  final Widget child;

  void _selectDestination(BuildContext context, int index) {
    if (index == selectedIndex) {
      return;
    }

    final route = switch (index) {
      0 => AppRoutes.foundation,
      1 => AppRoutes.chat,
      2 => AppRoutes.about,
      _ => throw ArgumentError.value(index, 'index', 'Unknown destination'),
    };
    Navigator.of(context).pushReplacementNamed(route);
  }

  @override
  Widget build(BuildContext context) {
    final localizations = AppLocalizations.of(context);

    return LayoutBuilder(
      builder: (context, constraints) {
        if (constraints.maxWidth >= _railBreakpoint) {
          return Scaffold(
            body: Row(
              children: [
                NavigationRail(
                  selectedIndex: selectedIndex,
                  onDestinationSelected: (index) =>
                      _selectDestination(context, index),
                  labelType: NavigationRailLabelType.all,
                  destinations: [
                    NavigationRailDestination(
                      icon: Icon(
                        Icons.dashboard_outlined,
                        key: AppKeys.foundationNavigation,
                        semanticLabel: localizations.foundationNavigationLabel,
                      ),
                      selectedIcon: Icon(
                        Icons.dashboard,
                        semanticLabel: localizations.foundationNavigationLabel,
                      ),
                      label: Text(localizations.foundationNavigationLabel),
                    ),
                    NavigationRailDestination(
                      icon: Icon(
                        Icons.forum_outlined,
                        key: AppKeys.chatNavigation,
                        semanticLabel: localizations.chatNavigationLabel,
                      ),
                      selectedIcon: Icon(
                        Icons.forum,
                        semanticLabel: localizations.chatNavigationLabel,
                      ),
                      label: Text(localizations.chatNavigationLabel),
                    ),
                    NavigationRailDestination(
                      icon: Icon(
                        Icons.info_outline,
                        key: AppKeys.aboutNavigation,
                        semanticLabel: localizations.aboutNavigationLabel,
                      ),
                      selectedIcon: Icon(
                        Icons.info,
                        semanticLabel: localizations.aboutNavigationLabel,
                      ),
                      label: Text(localizations.aboutNavigationLabel),
                    ),
                  ],
                ),
                const VerticalDivider(width: 1),
                Expanded(child: child),
              ],
            ),
          );
        }

        return Scaffold(
          body: child,
          bottomNavigationBar: NavigationBar(
            selectedIndex: selectedIndex,
            onDestinationSelected: (index) =>
                _selectDestination(context, index),
            destinations: [
              NavigationDestination(
                icon: Icon(
                  Icons.dashboard_outlined,
                  key: AppKeys.foundationNavigation,
                ),
                selectedIcon: const Icon(Icons.dashboard),
                label: localizations.foundationNavigationLabel,
              ),
              NavigationDestination(
                icon: Icon(Icons.info_outline, key: AppKeys.aboutNavigation),
                selectedIcon: const Icon(Icons.info),
                label: localizations.aboutNavigationLabel,
              ),
            ],
          ),
        );
      },
    );
  }
}
