DROP TABLE IF EXISTS `accounts_ipauth`;
CREATE TABLE IF NOT EXISTS `accounts_ipauth` (
  `login` varchar(45) NOT NULL,
  `ip` char(15) NOT NULL,
  `type` varchar(10) NULL DEFAULT 'allow'
);