DROP TABLE IF EXISTS `announcements`;
CREATE TABLE IF NOT EXISTS `announcements` (
  `id` INTEGER PRIMARY KEY AUTOINCREMENT,
  `type` int NOT NULL,
  `initial` bigint NOT NULL DEFAULT 0,
  `delay` bigint NOT NULL DEFAULT 0,
  `repeat` int NOT NULL DEFAULT 0,
  `author` text NOT NULL,
  `content` text NOT NULL
) ;

INSERT INTO announcements (`type`, `author`, `content`) VALUES 
(0, 'L2jMobius', 'Thanks for using L2jMobius!'),
(0, 'L2jMobius', '[=http://www.l2jmobius.org/=]');
